//! The client and its one HTTP transport.

use std::fmt;
use std::hash::{BuildHasher, Hash, Hasher, RandomState};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

use reqwest::Method;
use reqwest::header::{HeaderValue, LOCATION, USER_AGENT};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;

use crate::config::{self, Config, ConfigError, Credentials};
use crate::error::details;
use crate::{Error, ErrorKind, Response, ResponseMeta, Stage};

/// An endpoint: its method, and the path template that errors name in place
/// of the URL.
pub(crate) struct Endpoint {
    pub(crate) method: Method,
    pub(crate) template: &'static str,
}

/// A client for one region and environment. Cloning it is cheap and shares
/// its connections.
#[derive(Clone)]
pub struct Client(Arc<Inner>);

struct Inner {
    http: reqwest::Client,
    base_url: url::Url,
    key_id: HeaderValue,
    secret: HeaderValue,
    /// Kept to scrub them from anything Alpaca echoes back.
    credentials: Credentials,
    request_timeout: Duration,
    max_body_bytes: u64,
    read_deadline: Duration,
    retry_base: Duration,
    retry_cap: Duration,
    max_read_attempts: u32,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Client { .. }")
    }
}

fn sensitive_header(value: &SecretString) -> HeaderValue {
    let mut header = HeaderValue::from_str(value.expose_secret())
        .expect("credentials are checked to be printable ASCII");
    header.set_sensitive(true);
    header
}

/// The request URL: the base URL with each segment appended, encoded, so a
/// `/` inside a segment becomes `%2F` and never starts another segment.
fn request_url(base_url: &url::Url, segments: &[&str]) -> url::Url {
    let mut url = base_url.clone();
    url.path_segments_mut()
        .expect("an http or https base URL has a path")
        .extend(segments);
    url
}

/// Why a response body could not be read whole.
enum BodyError {
    TooLarge,
    Failed { timeout: bool },
}

/// Reads a whole response body, failing as soon as it exceeds `limit` bytes.
async fn read_body(response: &mut reqwest::Response, limit: u64) -> Result<Vec<u8>, BodyError> {
    let mut body = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if (body.len() + chunk.len()) as u64 > limit {
                    return Err(BodyError::TooLarge);
                }
                body.extend_from_slice(&chunk);
            }
            Ok(None) => return Ok(body),
            Err(e) => {
                return Err(BodyError::Failed {
                    timeout: e.is_timeout(),
                });
            }
        }
    }
}

/// The scheme, host and port of an absolute http or https `Location`, never
/// its credentials, path or query.
fn redirect_origin(location: Option<&HeaderValue>) -> Option<String> {
    let url = url::Url::parse(location?.to_str().ok()?).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let host = url.host_str()?;
    Some(match url.port() {
        Some(port) => format!("{}://{host}:{port}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    })
}

impl Client {
    /// Checks the configuration and builds the client.
    pub fn new(config: Config) -> Result<Client, ConfigError> {
        let base_url = config::check(&config)?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .retry(reqwest::retry::never())
            .http1_only()
            .no_gzip()
            .no_brotli()
            .no_zstd()
            .no_deflate()
            .pool_idle_timeout(Duration::from_secs(30))
            .connect_timeout(config.connect_timeout)
            .tls_backend_rustls()
            .tls_sslkeylogfile(false)
            .referer(false)
            .build()
            .map_err(|_| ConfigError::tls())?;
        Ok(Client(Arc::new(Inner {
            http,
            base_url,
            key_id: sensitive_header(&config.credentials.key_id),
            secret: sensitive_header(&config.credentials.secret),
            credentials: config.credentials,
            request_timeout: config.request_timeout,
            max_body_bytes: config.max_body_bytes,
            read_deadline: config.read_deadline,
            retry_base: config.retry_base,
            retry_cap: config.retry_cap,
            max_read_attempts: config.max_read_attempts,
        })))
    }

    /// Sends a read, retrying it within the read deadline, and decodes a
    /// successful response's JSON body. Every request so far is a read.
    pub(crate) async fn send<T: DeserializeOwned>(
        &self,
        endpoint: &Endpoint,
        segments: &[&str],
    ) -> Result<Response<T>, Error> {
        let inner = &self.0;
        let start = Instant::now();
        let mut attempt = 1;
        loop {
            let remaining = inner.read_deadline.saturating_sub(start.elapsed());
            let timeout = inner.request_timeout.min(remaining);
            let error = match self.attempt(endpoint, segments, attempt, timeout).await {
                Ok(response) => return Ok(response),
                Err(error) => error,
            };
            if attempt >= inner.max_read_attempts || !retryable(&error) {
                return Err(error);
            }
            let wait = self.wait(attempt, &error);
            if start.elapsed() + wait >= inner.read_deadline {
                return Err(error);
            }
            tokio::time::sleep(wait).await;
            attempt += 1;
        }
    }

    /// The wait after attempt `attempt` failed: the backoff, or the time
    /// until a 429's rate-limit reset if later, plus up to half the backoff
    /// at random, so that clients do not retry in step.
    fn wait(&self, attempt: u32, error: &Error) -> Duration {
        let inner = &self.0;
        let backoff = inner
            .retry_base
            .saturating_mul(1 << (attempt - 1))
            .min(inner.retry_cap);
        let reset = match error.meta() {
            Some(meta) if meta.status() == 429 => meta.reset(),
            _ => None,
        };
        let until_reset = reset
            .and_then(|reset| u64::try_from(reset.unix_seconds()).ok())
            .map(|seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
            .and_then(|reset| reset.duration_since(SystemTime::now()).ok())
            .unwrap_or_default();
        backoff.max(until_reset) + (backoff / 2).mul_f64(jitter(attempt))
    }

    /// One attempt of a request, with this attempt's number and timeout.
    async fn attempt<T: DeserializeOwned>(
        &self,
        endpoint: &Endpoint,
        segments: &[&str],
        attempt: u32,
        timeout: Duration,
    ) -> Result<Response<T>, Error> {
        let inner = &self.0;
        let error = |kind, stage| {
            Error::new(
                kind,
                stage,
                endpoint.method.clone(),
                endpoint.template,
                attempt,
            )
        };
        // reqwest's own errors are never kept: their text holds the URL.
        let sent = inner
            .http
            .request(
                endpoint.method.clone(),
                request_url(&inner.base_url, segments),
            )
            .header("APCA-API-KEY-ID", inner.key_id.clone())
            .header("APCA-API-SECRET-KEY", inner.secret.clone())
            .header(USER_AGENT, concat!("suri/", env!("CARGO_PKG_VERSION")))
            .timeout(timeout)
            .send()
            .await;
        let mut response = match sent {
            Ok(response) => response,
            Err(e) => {
                // Only a failure to build the request or to connect (DNS, TCP
                // or TLS) proves that no byte of it left.
                let stage = if e.is_builder() || e.is_connect() {
                    Stage::NotStarted
                } else {
                    Stage::Unknown
                };
                let kind = if e.is_timeout() {
                    ErrorKind::Timeout
                } else {
                    ErrorKind::Transport
                };
                return Err(error(kind, stage));
            }
        };
        let status = response.status().as_u16();
        let meta = ResponseMeta::from_headers(status, response.headers());
        if response.status().is_redirection() {
            let location = redirect_origin(response.headers().get(LOCATION));
            return Err(error(ErrorKind::Redirect, Stage::ResponseReceived)
                .with_meta(meta)
                .with_location(location));
        }
        let body = read_body(&mut response, inner.max_body_bytes).await;
        if !response.status().is_success() {
            // The status decides the kind; the body is read on a best-effort
            // basis for Alpaca's code and message.
            let credentials = [
                inner.credentials.key_id.expose_secret(),
                inner.credentials.secret.expose_secret(),
            ];
            let (code, message) = details(body.as_deref().ok(), &credentials);
            return Err(error(ErrorKind::Api, Stage::ResponseReceived)
                .with_meta(meta)
                .with_details(code, message));
        }
        let body = match body {
            Ok(body) => body,
            Err(BodyError::TooLarge) => {
                return Err(error(ErrorKind::BodyTooLarge, Stage::ResponseReceived).with_meta(meta));
            }
            Err(BodyError::Failed { timeout }) => {
                let kind = if timeout {
                    ErrorKind::Timeout
                } else {
                    ErrorKind::Transport
                };
                return Err(error(kind, Stage::ResponseReceived).with_meta(meta));
            }
        };
        match serde_json::from_slice(&body) {
            Ok(data) => Ok(Response::new(data, meta)),
            Err(_) => Err(error(ErrorKind::Decode, Stage::ResponseReceived).with_meta(meta)),
        }
    }
}

/// Whether a failed read is worth another attempt: a 429, a 500, 502, 503
/// or 504, or a failed or timed-out connection at any stage.
fn retryable(error: &Error) -> bool {
    match error.kind() {
        ErrorKind::Api => matches!(error.status(), Some(429 | 500 | 502 | 503 | 504)),
        ErrorKind::Transport | ErrorKind::Timeout => true,
        _ => false,
    }
}

/// A fraction in `[0, 1)`, different on every call.
fn jitter(attempt: u32) -> f64 {
    let mut hasher = RandomState::new().build_hasher();
    attempt.hash(&mut hasher);
    Instant::now().hash(&mut hasher);
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Symbol;

    fn decode(segment: &str) -> String {
        let bytes = segment.as_bytes();
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'%' {
                out.push(u8::from_str_radix(&segment[at + 1..at + 3], 16).unwrap());
                at += 3;
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_path_value_stays_one_encoded_segment() {
        for text in ["..", ".", "", ".\t.", "\n..", "AA\tPL"] {
            assert!(text.parse::<Symbol>().is_err(), "{text:?}");
        }
        let base = url::Url::parse("https://paper-api.alpaca.markets").unwrap();
        for (text, encoded) in [
            ("BTC/USD", "BTC%2FUSD"),
            ("BRK.B", "BRK.B"),
            ("%2e%2e", "%252e%252e"),
        ] {
            let symbol: Symbol = text.parse().unwrap();
            let url = request_url(&base, &["v2", "assets", symbol.as_str()]);
            assert_eq!(
                url.as_str(),
                format!("https://paper-api.alpaca.markets/v2/assets/{encoded}")
            );
            let last = url.path_segments().unwrap().next_back().unwrap();
            assert_eq!(decode(last), text);
        }
    }

    #[test]
    fn the_key_pair_headers_are_marked_sensitive() {
        let config = Config::new(
            crate::Region::US,
            crate::Environment::Paper,
            crate::Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
        );
        let client = Client::new(config).unwrap();
        assert!(client.0.key_id.is_sensitive());
        assert!(client.0.secret.is_sensitive());
    }

    fn client_with_backoff(base: Duration, cap: Duration) -> Client {
        let config = Config::new(
            crate::Region::US,
            crate::Environment::Paper,
            crate::Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
        )
        .with_retry_base(base)
        .with_retry_cap(cap);
        Client::new(config).unwrap()
    }

    fn api_error(status: u16, reset: Option<u64>) -> Error {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(reset) = reset {
            headers.insert("x-ratelimit-reset", HeaderValue::from(reset));
        }
        Error::new(
            ErrorKind::Api,
            Stage::ResponseReceived,
            Method::GET,
            "/v2/clock",
            1,
        )
        .with_meta(ResponseMeta::from_headers(status, &headers))
    }

    #[test]
    fn the_backoff_doubles_to_its_cap_with_up_to_half_added() {
        let ms = Duration::from_millis;
        let client = client_with_backoff(ms(100), ms(250));
        let error = api_error(503, None);
        for (attempt, backoff) in [(1, 100), (2, 200), (3, 250), (4, 250)] {
            for _ in 0..50 {
                let wait = client.wait(attempt, &error);
                assert!(wait >= ms(backoff), "{attempt}: {wait:?}");
                assert!(wait < ms(backoff) + ms(backoff) / 2, "{attempt}: {wait:?}");
            }
        }
        let waits: std::collections::HashSet<Duration> =
            (0..50).map(|_| client.wait(1, &error)).collect();
        assert!(waits.len() > 1, "no jitter: {waits:?}");
    }

    #[test]
    fn a_throttled_read_waits_until_the_reset() {
        let client = client_with_backoff(Duration::from_millis(100), Duration::from_secs(1));
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let wait = client.wait(1, &api_error(429, Some(now + 10)));
        assert!(wait >= Duration::from_secs(9), "{wait:?}");
        // A reset on another status is not a reason to wait.
        let wait = client.wait(1, &api_error(503, Some(now + 10)));
        assert!(wait < Duration::from_millis(150), "{wait:?}");
    }

    #[test]
    fn endpoint_paths_join_the_base_url() {
        for base in ["https://h", "https://h/", "http://127.0.0.1:1/"] {
            let base = url::Url::parse(base).unwrap();
            assert_eq!(request_url(&base, &["v2", "clock"]).path(), "/v2/clock");
        }
    }
}
