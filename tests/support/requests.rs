//! Clients and servers for request tests: the sentinel key pair, a client
//! that tries once and one that retries, single-response servers, and the
//! checks every request test makes.

#![allow(dead_code, reason = "each test binary uses part of the helper")]

use std::time::Duration;

use suri::{Client, Config, Credentials, Environment, Region};

use crate::server::{Request, Server, Step, response};

/// The key ID every test client sends.
pub const KEY: &str = "PKSENTINELKEY0001";
/// The secret every test client sends; tests look for it where it must not
/// appear.
pub const SECRET: &str = "SECRETSENTINELVALUE0001";

/// A configuration pointing at the server, with the sentinel key pair.
pub fn config(server: &Server) -> Config {
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key(KEY, SECRET),
    )
    .with_base_url(&server.url)
}

/// A client that makes one attempt per request.
pub fn client(server: &Server) -> Client {
    Client::new(config(server).with_max_read_attempts(1)).unwrap()
}

/// A client with the default four attempts, so a read in the same
/// situation would reach a server's second step.
pub fn retrying_client(server: &Server) -> Client {
    Client::new(config(server).with_retry_base(Duration::from_millis(1))).unwrap()
}

/// A server that answers one request with this status and body.
pub fn serve(status: &str, body: &[u8]) -> Server {
    Server::start(vec![Step::Respond(response(status, &[], body))])
}

/// A server that fails the first request with a 503 and keeps a success for
/// a second one, which a retry would reach.
pub fn failing_once(spare: &[u8]) -> Server {
    Server::start(vec![
        Step::Respond(response("503 Service Unavailable", &[], b"")),
        Step::Respond(response("200 OK", &[], spare)),
    ])
}

/// The one request the server received.
pub fn only_request(server: &Server) -> Request {
    let mut requests = server.requests();
    assert_eq!(requests.len(), 1);
    requests.remove(0)
}

/// The request carried no body and no `Content-Type`.
pub fn assert_no_body(request: &Request) {
    assert_eq!(request.header("Content-Type"), None);
    assert!(request.body.is_empty());
}

/// A POST without a body states its length as zero.
pub fn assert_empty_post(request: &Request) {
    assert_eq!(request.method, "POST");
    assert_no_body(request);
    assert_eq!(request.header("Content-Length"), Some("0"));
}
