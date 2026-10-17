//! The client and its one HTTP transport.

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use reqwest::Method;
use reqwest::header::{HeaderValue, LOCATION, USER_AGENT};
use secrecy::{ExposeSecret, SecretString};
use serde::de::DeserializeOwned;

use crate::config::{self, Config, ConfigError};
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
    request_timeout: Duration,
    max_body_bytes: u64,
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
            request_timeout: config.request_timeout,
            max_body_bytes: config.max_body_bytes,
        })))
    }

    /// Sends one request and decodes a successful response's JSON body.
    pub(crate) async fn send<T: DeserializeOwned>(
        &self,
        endpoint: &Endpoint,
        segments: &[&str],
    ) -> Result<Response<T>, Error> {
        let inner = &self.0;
        let error =
            |kind, stage| Error::new(kind, stage, endpoint.method.clone(), endpoint.template, 1);
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
            .timeout(inner.request_timeout)
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
        if !response.status().is_success() {
            return Err(error(ErrorKind::Api, Stage::ResponseReceived).with_meta(meta));
        }
        let mut body = Vec::new();
        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if (body.len() + chunk.len()) as u64 > inner.max_body_bytes {
                        return Err(
                            error(ErrorKind::BodyTooLarge, Stage::ResponseReceived).with_meta(meta)
                        );
                    }
                    body.extend_from_slice(&chunk);
                }
                Ok(None) => break,
                Err(e) => {
                    let kind = if e.is_timeout() {
                        ErrorKind::Timeout
                    } else {
                        ErrorKind::Transport
                    };
                    return Err(error(kind, Stage::ResponseReceived).with_meta(meta));
                }
            }
        }
        match serde_json::from_slice(&body) {
            Ok(data) => Ok(Response::new(data, meta)),
            Err(_) => Err(error(ErrorKind::Decode, Stage::ResponseReceived).with_meta(meta)),
        }
    }
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

    #[test]
    fn endpoint_paths_join_the_base_url() {
        for base in ["https://h", "https://h/", "http://127.0.0.1:1/"] {
            let base = url::Url::parse(base).unwrap();
            assert_eq!(request_url(&base, &["v2", "clock"]).path(), "/v2/clock");
        }
    }
}
