//! Where requests go, with which credentials, and the limits they run under.

use std::fmt;
use std::time::Duration;

use secrecy::{ExposeSecret, SecretString};

/// The Alpaca region whose hosts a client uses.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Region {
    /// Alpaca's US hosts.
    US,
}

/// Paper (simulated) or live trading.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Environment {
    /// The paper-trading host.
    Paper,
    /// The live-trading host.
    Live,
}

/// The Trading API host for a region and environment. The version prefix is
/// part of each endpoint's path, not of the host.
pub(crate) fn trading_host(region: Region, environment: Environment) -> &'static str {
    match (region, environment) {
        (Region::US, Environment::Paper) => "https://paper-api.alpaca.markets",
        (Region::US, Environment::Live) => "https://api.alpaca.markets",
    }
}

/// An API key pair: the key ID and its secret.
///
/// Neither value can be read back, and `Debug` prints `Credentials { .. }`.
#[derive(Clone)]
pub struct Credentials {
    pub(crate) key_id: SecretString,
    pub(crate) secret: SecretString,
}

impl Credentials {
    /// The key ID and secret key Alpaca issues for the Trading API.
    pub fn api_key(key_id: impl Into<String>, secret: impl Into<String>) -> Credentials {
        Credentials {
            key_id: SecretString::from(key_id.into()),
            secret: SecretString::from(secret.into()),
        }
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credentials { .. }")
    }
}

/// Everything a [`Client`](crate::Client) needs, checked by
/// [`Client::new`](crate::Client::new).
///
/// `Debug` prints the credentials as `Credentials { .. }` and a base URL as
/// `Some(..)`, since a key pair mistakenly written into a URL must not print.
#[derive(Clone)]
pub struct Config {
    pub(crate) region: Region,
    pub(crate) environment: Environment,
    pub(crate) credentials: Credentials,
    pub(crate) connect_timeout: Duration,
    pub(crate) request_timeout: Duration,
    pub(crate) max_body_bytes: u64,
    pub(crate) base_url: Option<String>,
}

impl Config {
    /// A configuration with the documented defaults: a 10 s connect timeout,
    /// a 30 s request timeout and a 32 MiB response body limit.
    pub fn new(region: Region, environment: Environment, credentials: Credentials) -> Config {
        Config {
            region,
            environment,
            credentials,
            connect_timeout: Duration::from_secs(10),
            request_timeout: Duration::from_secs(30),
            max_body_bytes: 32 * 1024 * 1024,
            base_url: None,
        }
    }

    /// The longest wait for a connection, TLS included (1 ms to 60 s, and no
    /// longer than the request timeout).
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// The longest one request may take, from connecting until the whole
    /// response body has arrived (1 ms to 10 min).
    pub fn with_request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// The largest response body read (1 KiB to 1 GiB).
    pub fn with_max_body_bytes(mut self, bytes: u64) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Sends requests to this URL instead of the region's host, for tests
    /// against a local server. It must use https unless its host is loopback,
    /// and must have no path, query, fragment or credentials.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("region", &self.region)
            .field("environment", &self.environment)
            .field("credentials", &self.credentials)
            .field("connect_timeout", &self.connect_timeout)
            .field("request_timeout", &self.request_timeout)
            .field("max_body_bytes", &self.max_body_bytes)
            .field("base_url", &self.base_url.as_ref().map(|_| ..))
            .finish()
    }
}

/// The error returned when a [`Config`] cannot make a client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigError {
    kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    CredentialsEmpty,
    CredentialsBytes,
    BaseUrlInvalid,
    BaseUrlScheme,
    BaseUrlPath,
    BaseUrlCredentials,
    ConnectTimeout,
    RequestTimeout,
    ConnectLongerThanRequest,
    MaxBodyBytes,
    Tls,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            Kind::CredentialsEmpty => "credentials must not be empty",
            Kind::CredentialsBytes => "credentials must be printable ASCII",
            Kind::BaseUrlInvalid => "base URL is not a valid URL",
            Kind::BaseUrlScheme => "base URL must use https unless the host is loopback",
            Kind::BaseUrlPath => "base URL must not have a path, query or fragment",
            Kind::BaseUrlCredentials => "base URL must not contain credentials",
            Kind::ConnectTimeout => "connect_timeout must be between 1ms and 60s",
            Kind::RequestTimeout => "request_timeout must be between 1ms and 10min",
            Kind::ConnectLongerThanRequest => {
                "connect_timeout must not be longer than request_timeout"
            }
            Kind::MaxBodyBytes => "max_body_bytes must be between 1KiB and 1GiB",
            Kind::Tls => "the TLS client could not be built",
        })
    }
}

impl std::error::Error for ConfigError {}

impl ConfigError {
    pub(crate) fn tls() -> ConfigError {
        ConfigError { kind: Kind::Tls }
    }
}

fn fail<T>(kind: Kind) -> Result<T, ConfigError> {
    Err(ConfigError { kind })
}

fn check_credential(value: &SecretString) -> Result<(), ConfigError> {
    let text = value.expose_secret();
    if text.is_empty() {
        return fail(Kind::CredentialsEmpty);
    }
    if !text.bytes().all(|b| (0x21..=0x7e).contains(&b)) {
        return fail(Kind::CredentialsBytes);
    }
    Ok(())
}

fn is_loopback(url: &url::Url) -> bool {
    match url.host() {
        Some(url::Host::Domain(name)) => name.eq_ignore_ascii_case("localhost"),
        Some(url::Host::Ipv4(address)) => address.is_loopback(),
        Some(url::Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    }
}

fn check_base_url(text: &str) -> Result<url::Url, ConfigError> {
    let Ok(url) = url::Url::parse(text) else {
        return fail(Kind::BaseUrlInvalid);
    };
    if !(url.scheme() == "https" || url.scheme() == "http" && is_loopback(&url)) {
        return fail(Kind::BaseUrlScheme);
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return fail(Kind::BaseUrlPath);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return fail(Kind::BaseUrlCredentials);
    }
    Ok(url)
}

/// Checks the credentials, then the base URL, then each policy value in the
/// contract's table order; the first failure is reported. Returns the base
/// URL requests go to.
pub(crate) fn check(config: &Config) -> Result<url::Url, ConfigError> {
    check_credential(&config.credentials.key_id)?;
    check_credential(&config.credentials.secret)?;
    let base_url = match &config.base_url {
        Some(text) => check_base_url(text)?,
        None => url::Url::parse(trading_host(config.region, config.environment))
            .expect("the host table holds valid URLs"),
    };
    let between = |value, low, high| (low..=high).contains(&value);
    let ms = Duration::from_millis;
    if !between(config.connect_timeout, ms(1), Duration::from_secs(60)) {
        return fail(Kind::ConnectTimeout);
    }
    if !between(config.request_timeout, ms(1), Duration::from_secs(600)) {
        return fail(Kind::RequestTimeout);
    }
    if config.connect_timeout > config.request_timeout {
        return fail(Kind::ConnectLongerThanRequest);
    }
    if !(1024..=1024 * 1024 * 1024).contains(&config.max_body_bytes) {
        return fail(Kind::MaxBodyBytes);
    }
    Ok(base_url)
}
