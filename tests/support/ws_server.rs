//! A loopback WebSocket server that plays a script: one step per accepted
//! connection, in order, then it stops listening. It runs its own
//! single-threaded runtime on a thread named `ws-server`, so every record it
//! causes in a log comes from that thread, and it records the upgrade
//! request and every frame it reads.

#![allow(dead_code, reason = "each test binary uses part of the helper")]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::Frame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::{CloseCode, Data, OpCode};

/// What the server does with one connection.
pub enum Step {
    /// Answers the upgrade with this HTTP status and no WebSocket.
    Reject(u16),
    /// Accepts the upgrade, adding a `Sec-WebSocket-Extensions` header, then
    /// plays the acts.
    UpgradeWithExtension(Vec<Act>),
    /// Accepts the upgrade and plays the acts.
    Upgrade(Vec<Act>),
    /// Accepts the TCP connection and closes it at once.
    CloseTcp,
    /// Accepts the TCP connection and never answers the upgrade.
    Silent,
}

/// One act on an upgraded connection.
pub enum Act {
    /// Reads one frame of any kind and records it.
    Read,
    /// Reads and records frames until it has read a Ping.
    ReadUntilPing,
    /// Reads and records frames for this long, answering Pings.
    ReadFor(Duration),
    /// Reads and records frames until the client ends the connection.
    ReadAll,
    /// Sends this message.
    Send(Message),
    /// Sends a text frame whose payload is these bytes, valid UTF-8 or not.
    SendRawText(Vec<u8>),
    /// Sends one text message split into frames with these payloads.
    SendFragments(Vec<String>),
    /// Sends a Close frame with this code and reason.
    Close(u16, String),
    /// Sends a Close frame without a payload.
    CloseEmpty,
    /// Waits without reading, so Pings go unanswered.
    Pause(Duration),
    /// Drops the connection without a Close frame.
    Drop,
}

/// A frame the server read.
#[derive(Debug, Clone, PartialEq)]
pub enum Received {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close(Option<(u16, String)>),
}

/// What the server saw on one connection.
#[derive(Debug, Clone, Default)]
pub struct Seen {
    pub path: String,
    /// Header names in lower case, in the order sent.
    pub headers: Vec<(String, String)>,
    pub frames: Vec<Received>,
}

impl Seen {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// The text frames read, in order.
    pub fn texts(&self) -> Vec<&str> {
        self.frames
            .iter()
            .filter_map(|frame| match frame {
                Received::Text(text) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }
}

pub struct WsServer {
    /// The base URL to give the client: `http://127.0.0.1:<port>`.
    pub url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
}

impl WsServer {
    pub fn start(steps: Vec<Step>) -> WsServer {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let recorded = seen.clone();
        std::thread::Builder::new()
            .name("ws-server".to_owned())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let listener = TcpListener::from_std(listener).unwrap();
                    for step in steps {
                        let Ok((tcp, _)) = listener.accept().await else {
                            return;
                        };
                        let index = {
                            let mut seen = recorded.lock().unwrap();
                            seen.push(Seen::default());
                            seen.len() - 1
                        };
                        let recorded = recorded.clone();
                        tokio::spawn(play(tcp, step, recorded, index));
                    }
                    // Keep serving the spawned connections.
                    std::future::pending::<()>().await;
                });
            })
            .unwrap();
        WsServer { url, seen }
    }

    /// What the server saw, one entry per connection accepted.
    pub fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }

    /// Waits until the server has seen `count` connections, each with at
    /// least the given number of frames on the last one.
    pub async fn wait_for_frames(&self, connection: usize, frames: usize) -> Seen {
        for _ in 0..500 {
            if let Some(seen) = self.seen().get(connection)
                && seen.frames.len() >= frames
            {
                return seen.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("the server did not read {frames} frames: {:?}", self.seen());
    }
}

/// The opening the client is expected to perform, answered successfully.
pub fn opening() -> Vec<Act> {
    vec![
        Act::Read,
        Act::Send(Message::text(
            r#"{"stream":"authorization","data":{"status":"authorized","action":"authenticate"}}"#,
        )),
        Act::Read,
        Act::Send(Message::text(
            r#"{"stream":"listening","data":{"streams":["trade_updates"]}}"#,
        )),
    ]
}

/// The opening followed by the given acts.
pub fn opened(acts: Vec<Act>) -> Step {
    let mut all = opening();
    all.extend(acts);
    Step::Upgrade(all)
}

// The handshake callbacks return tungstenite's own error response type.
#[allow(clippy::result_large_err)]
async fn play(tcp: tokio::net::TcpStream, step: Step, seen: Arc<Mutex<Vec<Seen>>>, index: usize) {
    let (acts, extension) = match step {
        Step::CloseTcp => return,
        Step::Silent => {
            let _tcp = tcp;
            std::future::pending::<()>().await;
            return;
        }
        Step::Reject(status) => {
            let callback = |request: &Request, _: Response| {
                record_request(&seen, index, request);
                let mut response = ErrorResponse::new(None);
                *response.status_mut() = StatusCode::from_u16(status).unwrap();
                Err(response)
            };
            let _ = tokio_tungstenite::accept_hdr_async(tcp, callback).await;
            return;
        }
        Step::Upgrade(acts) => (acts, false),
        Step::UpgradeWithExtension(acts) => (acts, true),
    };
    let callback = |request: &Request, mut response: Response| {
        record_request(&seen, index, request);
        if extension {
            response.headers_mut().insert(
                "sec-websocket-extensions",
                HeaderValue::from_static("permessage-deflate"),
            );
        }
        Ok(response)
    };
    let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(tcp, callback).await else {
        return;
    };
    let record = |frame: Received| seen.lock().unwrap()[index].frames.push(frame);
    for act in acts {
        match act {
            Act::Read => {
                if let Some(Ok(message)) = ws.next().await {
                    record(received(message));
                }
            }
            Act::ReadUntilPing => {
                while let Some(Ok(message)) = ws.next().await {
                    let is_ping = matches!(message, Message::Ping(_));
                    record(received(message));
                    if is_ping {
                        break;
                    }
                }
            }
            Act::ReadFor(duration) => {
                let until = tokio::time::Instant::now() + duration;
                while let Ok(Some(Ok(message))) = tokio::time::timeout_at(until, ws.next()).await {
                    record(received(message));
                }
                // A Pong queued by the last read goes out now.
                let _ = ws.flush().await;
            }
            Act::ReadAll => {
                while let Some(Ok(message)) = ws.next().await {
                    record(received(message));
                }
            }
            Act::Send(message) => {
                let _ = ws.send(message).await;
            }
            Act::SendRawText(bytes) => {
                let frame = Frame::message(bytes, OpCode::Data(Data::Text), true);
                let _ = ws.send(Message::Frame(frame)).await;
            }
            Act::SendFragments(parts) => {
                let last = parts.len() - 1;
                for (at, part) in parts.into_iter().enumerate() {
                    let opcode = if at == 0 {
                        OpCode::Data(Data::Text)
                    } else {
                        OpCode::Data(Data::Continue)
                    };
                    let frame = Frame::message(part.into_bytes(), opcode, at == last);
                    let _ = ws.send(Message::Frame(frame)).await;
                }
            }
            Act::Close(code, reason) => {
                let frame = CloseFrame {
                    code: CloseCode::from(code),
                    reason: reason.into(),
                };
                let _ = ws.send(Message::Close(Some(frame))).await;
            }
            Act::CloseEmpty => {
                let _ = ws.send(Message::Close(None)).await;
            }
            Act::Pause(duration) => tokio::time::sleep(duration).await,
            Act::Drop => return,
        }
    }
    // Hold the connection open until the client ends it.
    while let Some(Ok(message)) = ws.next().await {
        record(received(message));
    }
}

fn record_request(seen: &Mutex<Vec<Seen>>, index: usize, request: &Request) {
    let mut seen = seen.lock().unwrap();
    seen[index].path = request.uri().path().to_owned();
    seen[index].headers = request
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                value.to_str().unwrap_or("").to_owned(),
            )
        })
        .collect();
}

fn received(message: Message) -> Received {
    match message {
        Message::Text(text) => Received::Text(text.as_str().to_owned()),
        Message::Binary(bytes) => Received::Binary(bytes.to_vec()),
        Message::Ping(bytes) => Received::Ping(bytes.to_vec()),
        Message::Pong(bytes) => Received::Pong(bytes.to_vec()),
        Message::Close(frame) => Received::Close(
            frame.map(|frame| (u16::from(frame.code), frame.reason.as_str().to_owned())),
        ),
        Message::Frame(_) => Received::Binary(Vec::new()),
    }
}
