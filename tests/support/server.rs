//! A loopback HTTP/1.1 server that plays a script: one step per accepted
//! connection, in order, then it stops listening, so any further connection
//! is refused. It records every request it reads.

#![allow(dead_code, reason = "each test binary uses part of the helper")]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime};

/// What the server does with one connection.
pub enum Step {
    /// Reads the request and writes these bytes, then closes.
    Respond(Vec<u8>),
    /// Reads the request and never answers, until the client closes.
    Hold,
    /// Reads the request and closes without writing a byte.
    Drop,
    /// Reads the request, writes these bytes (a head and part of a body),
    /// then closes.
    Partial(Vec<u8>),
    /// Reads the request, writes these bytes, then holds until the client
    /// closes.
    PartialHold(Vec<u8>),
}

/// One request as the server read it.
#[derive(Debug, Clone)]
pub struct Request {
    pub at: Instant,
    /// The wall-clock arrival, to compare with a rate-limit reset.
    pub wall: SystemTime,
    pub method: String,
    pub path: String,
    /// Names in lower case, in the order sent.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

pub struct Server {
    pub url: String,
    requests: Arc<Mutex<Vec<Request>>>,
}

impl Server {
    pub fn start(steps: Vec<Step>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = requests.clone();
        std::thread::spawn(move || {
            for step in steps {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let recorded = recorded.clone();
                std::thread::spawn(move || play(stream, step, &recorded));
            }
        });
        Server { url, requests }
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }
}

/// The URL of a loopback port nothing listens on.
pub fn refused_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    format!("http://{}", listener.local_addr().unwrap())
}

/// A complete response with `Content-Length` and `Connection: close`.
pub fn response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    )
    .into_bytes();
    for (name, value) in headers {
        bytes.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    bytes.extend_from_slice(b"\r\n");
    bytes.extend_from_slice(body);
    bytes
}

fn play(mut stream: TcpStream, step: Step, recorded: &Mutex<Vec<Request>>) {
    let Some(request) = read_request(&mut stream) else {
        return;
    };
    recorded.lock().unwrap().push(request);
    match step {
        Step::Respond(bytes) | Step::Partial(bytes) => {
            let _ = stream.write_all(&bytes);
        }
        Step::Hold => hold(&mut stream),
        Step::Drop => {}
        Step::PartialHold(bytes) => {
            let _ = stream.write_all(&bytes);
            hold(&mut stream);
        }
    }
}

/// Waits until the client closes the connection.
fn hold(stream: &mut TcpStream) {
    let mut buffer = [0; 1024];
    while matches!(stream.read(&mut buffer), Ok(n) if n > 0) {}
}

fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut head = Vec::new();
    let mut byte = [0; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if stream.read(&mut byte).ok()? == 0 {
            return None;
        }
        head.push(byte[0]);
    }
    let at = Instant::now();
    let wall = SystemTime::now();
    let head = String::from_utf8_lossy(&head).into_owned();
    let mut lines = head.split("\r\n");
    let mut request_line = lines.next()?.split(' ');
    let method = request_line.next()?.to_owned();
    let path = request_line.next()?.to_owned();
    let headers: Vec<(String, String)> = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse().ok())
        .unwrap_or(0);
    let mut body = vec![0; length];
    stream.read_exact(&mut body).ok()?;
    Some(Request {
        at,
        wall,
        method,
        path,
        headers,
        body,
    })
}
