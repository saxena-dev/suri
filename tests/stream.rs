//! The trade-updates stream: the opening on the wire, raw messages, every
//! end loud and scrubbed, liveness, cancellation and the close.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use std::time::Duration;

use suri::{
    Client, Config, Credentials, Environment, Region, StreamErrorKind, StreamMessage,
    TradeStreamMessage, TradeUpdates,
};
use tokio_tungstenite::tungstenite::Message;
use ws_server::{Act, Received, Step, WsServer, opened, opening};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";
const AUTH: &str =
    r#"{"action":"auth","key":"PKSENTINELKEY0001","secret":"SECRETSENTINELVALUE0001"}"#;
const LISTEN: &str = r#"{"action":"listen","data":{"streams":["trade_updates"]}}"#;
const UPDATE: &str = r#"{"stream":"trade_updates","data":{"event":"new","order":{"id":"1"}}}"#;
const UPDATE_DATA: &str = r#"{"event":"new","order":{"id":"1"}}"#;

fn config(server: &WsServer) -> Config {
    Config::new(
        Region::US,
        Environment::Paper,
        Credentials::api_key(KEY, SECRET),
    )
    .with_base_url(&server.url)
}

fn client(server: &WsServer) -> Client {
    Client::new(config(server)).unwrap()
}

/// A client whose request timeout (and connect timeout) is short.
fn quick(server: &WsServer, timeout: Duration) -> Client {
    Client::new(
        config(server)
            .with_connect_timeout(timeout)
            .with_request_timeout(timeout),
    )
    .unwrap()
}

async fn open(server: &WsServer) -> TradeUpdates {
    client(server).trade_updates().await.unwrap()
}

fn assert_send<T: Send>(_: &T) {}

/// The message as sent, from a stream message suri did not type.
fn raw(message: TradeStreamMessage) -> StreamMessage {
    match message {
        TradeStreamMessage::Undecoded(message) | TradeStreamMessage::Other(message) => message,
        other => panic!("a typed message: {other:?}"),
    }
}

#[test]
fn the_stream_and_its_futures_are_send() {
    fn is_send<T: Send>() {}
    is_send::<TradeUpdates>();
    is_send::<suri::StreamError>();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = WsServer::start(vec![opened(vec![])]);
    runtime.block_on(async {
        let mut stream = open(&server).await;
        assert_send(&stream.next());
        assert_send(&stream.close());
    });
}

#[tokio::test]
async fn the_opening_is_sent_as_documented() {
    let server = WsServer::start(vec![opened(vec![Act::Send(Message::text(UPDATE))])]);
    let mut stream = open(&server).await;
    stream.next().await.unwrap();
    let seen = &server.seen()[0];
    assert_eq!(seen.path, "/stream");
    assert_eq!(
        seen.header("user-agent"),
        Some(concat!("suri/", env!("CARGO_PKG_VERSION")))
    );
    assert_eq!(seen.header("sec-websocket-extensions"), None);
    assert_eq!(seen.texts(), [AUTH, LISTEN]);
}

#[tokio::test]
async fn text_and_binary_frames_are_delivered_raw() {
    let other = r#"{"stream":"other","data":[1]}"#;
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::text(UPDATE)),
        Act::Send(Message::binary(UPDATE.as_bytes().to_vec())),
        Act::Send(Message::text(other)),
        Act::Send(Message::text(r#"{"stream":"other","data":null}"#)),
    ])]);
    let mut stream = open(&server).await;
    for _ in 0..2 {
        let message = stream.next().await.unwrap();
        // The minimal order is not a full order, so the update is kept raw.
        assert!(matches!(message, TradeStreamMessage::Undecoded(_)));
        let message = raw(message);
        assert_eq!(message.stream, "trade_updates");
        assert_eq!(message.data, UPDATE_DATA);
    }
    let message = stream.next().await.unwrap();
    assert!(matches!(message, TradeStreamMessage::Other(_)));
    let message = raw(message);
    assert_eq!(message.stream, "other");
    assert_eq!(message.data, "[1]");
    let message = raw(stream.next().await.unwrap());
    assert_eq!(message.stream, "other");
    assert_eq!(message.data, "null");
}

fn reply(text: &str) -> Act {
    Act::Send(Message::text(text.to_owned()))
}

#[tokio::test]
async fn rejected_credentials_end_the_opening() {
    let unauthorized =
        r#"{"stream":"authorization","data":{"status":"unauthorized","action":"authenticate"}}"#;
    let server = WsServer::start(vec![Step::Upgrade(vec![Act::Read, reply(unauthorized)])]);
    let error = client(&server).trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Unauthorized);
    assert!(error.to_string().contains("the stream did not open"));
    assert!(!error.to_string().contains("later messages"));
    // No listen was sent after the rejection.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(server.seen()[0].texts(), [AUTH]);

    let listen_rejected =
        r#"{"stream":"authorization","data":{"status":"unauthorized","action":"listen"}}"#;
    let authorized =
        r#"{"stream":"authorization","data":{"status":"authorized","action":"authenticate"}}"#;
    let server = WsServer::start(vec![Step::Upgrade(vec![
        Act::Read,
        reply(authorized),
        Act::Read,
        reply(listen_rejected),
    ])]);
    let error = client(&server).trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Unauthorized);
}

#[tokio::test]
async fn a_close_during_the_opening_is_closed() {
    let server = WsServer::start(vec![Step::Upgrade(vec![
        Act::Read,
        Act::Close(1008, "policy".to_owned()),
        Act::ReadAll,
    ])]);
    let error = client(&server).trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), Some(1008));
    assert_eq!(error.message(), Some("policy"));
    // The Close is answered before the connection is dropped.
    let seen = server.wait_for_frames(0, 2).await;
    assert_eq!(
        seen.frames[1],
        Received::Close(Some((1008, "policy".to_owned())))
    );
}

#[tokio::test]
async fn an_unanswered_upgrade_times_out_within_the_connect_timeout() {
    let server = WsServer::start(vec![Step::Silent]);
    let client = Client::new(
        config(&server)
            .with_connect_timeout(Duration::from_millis(200))
            .with_request_timeout(Duration::from_secs(5)),
    )
    .unwrap();
    let started = std::time::Instant::now();
    let error = client.trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[tokio::test]
async fn a_refused_upgrade_is_a_handshake_error() {
    for status in [401, 403, 429] {
        let server = WsServer::start(vec![Step::Reject(status)]);
        let error = client(&server).trade_updates().await.unwrap_err();
        assert_eq!(error.kind(), StreamErrorKind::Handshake, "{status}");
        assert_eq!(error.status(), Some(status));
        assert_eq!(server.seen().len(), 1);
    }
    let server = WsServer::start(vec![Step::UpgradeWithExtension(opening())]);
    let error = client(&server).trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Handshake);
    assert_eq!(error.status(), Some(101));
}

#[tokio::test]
async fn an_acknowledgement_without_trade_updates_is_not_listening() {
    let authorized =
        r#"{"stream":"authorization","data":{"status":"authorized","action":"authenticate"}}"#;
    let server = WsServer::start(vec![Step::Upgrade(vec![
        Act::Read,
        reply(authorized),
        Act::Read,
        reply(r#"{"stream":"listening","data":{"streams":[]}}"#),
    ])]);
    let error = client(&server).trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::NotListening);
}

// real-time: the opening's bound runs on a real socket.
#[tokio::test]
async fn an_unanswered_opening_times_out() {
    let server = WsServer::start(vec![Step::Upgrade(vec![
        Act::Read,
        Act::Pause(Duration::from_secs(3)),
    ])]);
    let error = quick(&server, Duration::from_secs(1))
        .trade_updates()
        .await
        .unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
}

#[tokio::test]
async fn a_close_after_the_opening_is_closed_scrubbed_and_final() {
    // The reason holds the secret; then a reason cut to 123 bytes that ends
    // in the secret's first 9 characters.
    let cut = format!("{}{}", "x".repeat(123 - 9), &SECRET[..9]);
    for reason in [format!("bad secret {SECRET}"), cut] {
        let server = WsServer::start(vec![opened(vec![
            Act::Close(1008, reason.clone()),
            Act::ReadAll,
        ])]);
        let mut stream = open(&server).await;
        let error = stream.next().await.unwrap_err();
        assert_eq!(error.kind(), StreamErrorKind::Closed);
        assert_eq!(error.close_code(), Some(1008));
        let message = error.message().unwrap();
        assert!(message.contains("[REDACTED]"), "{message}");
        for text in [message.to_owned(), error.to_string(), format!("{error:?}")] {
            assert!(
                !text.contains(SECRET) && !text.contains(&SECRET[..9]),
                "{text}"
            );
        }
        assert!(error.to_string().contains("later messages may be lost"));
        assert_eq!(
            stream.next().await.unwrap_err().kind(),
            StreamErrorKind::Ended
        );
        // The client answered the server's Close.
        let seen = server.wait_for_frames(0, 3).await;
        assert_eq!(
            seen.frames[2],
            Received::Close(Some((1008, reason.clone())))
        );
    }

    let server = WsServer::start(vec![opened(vec![Act::CloseEmpty, Act::ReadAll])]);
    let mut stream = open(&server).await;
    let error = stream.next().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), None);
}

#[tokio::test]
async fn an_error_action_is_a_server_error() {
    let text = format!(r#"{{"action":"error","data":{{"error_message":"oops {SECRET}"}}}}"#);
    let server = WsServer::start(vec![opened(vec![Act::Send(Message::text(text))])]);
    let mut stream = open(&server).await;
    let error = stream.next().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Server);
    assert_eq!(error.message(), Some("oops [REDACTED]"));
}

#[tokio::test]
async fn a_dropped_connection_is_a_transport_error() {
    let server = WsServer::start(vec![opened(vec![Act::Drop])]);
    let mut stream = open(&server).await;
    let error = stream.next().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Transport);
    assert!(error.to_string().contains("later messages may be lost"));
}

#[tokio::test]
async fn a_frame_that_is_not_a_message_is_a_decode_error() {
    for act in [
        Act::Send(Message::text("not json")),
        Act::Send(Message::text(r#"{"data":{}}"#)),
        Act::SendRawText(vec![b'{', 0xff, b'}']),
    ] {
        let server = WsServer::start(vec![opened(vec![act])]);
        let mut stream = open(&server).await;
        let error = stream.next().await.unwrap_err();
        assert_eq!(error.kind(), StreamErrorKind::Decode);
    }
}

#[tokio::test]
async fn messages_up_to_the_limit_are_delivered() {
    // A message of exactly 1,024 bytes, then one of 1,025.
    let frame = |length: usize| {
        let base = r#"{"stream":"trade_updates","data":""}"#;
        let fill = "a".repeat(length - base.len());
        format!(r#"{{"stream":"trade_updates","data":"{fill}"}}"#)
    };
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::text(frame(1024))),
        Act::Send(Message::text(frame(1025))),
    ])]);
    let client = Client::new(config(&server).with_max_body_bytes(1024)).unwrap();
    let mut stream = client.trade_updates().await.unwrap();
    assert_eq!(raw(stream.next().await.unwrap()).stream, "trade_updates");
    assert_eq!(
        stream.next().await.unwrap_err().kind(),
        StreamErrorKind::TooLarge
    );
}

#[tokio::test]
async fn a_message_over_the_limit_in_small_frames_is_too_large() {
    // Two frames of 600 bytes each make one message of 1,200 bytes.
    let text = format!(
        r#"{{"stream":"trade_updates","data":"{}"}}"#,
        "a".repeat(1200 - 36)
    );
    assert_eq!(text.len(), 1200);
    let server = WsServer::start(vec![opened(vec![Act::SendFragments(vec![
        text[..600].to_owned(),
        text[600..].to_owned(),
    ])])]);
    let client = Client::new(config(&server).with_max_body_bytes(1024)).unwrap();
    let mut stream = client.trade_updates().await.unwrap();
    assert_eq!(
        stream.next().await.unwrap_err().kind(),
        StreamErrorKind::TooLarge
    );
}

#[tokio::test]
async fn a_server_ping_is_answered_and_never_delivered() {
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::Ping(b"p1".to_vec().into())),
        Act::Send(Message::text(UPDATE)),
        Act::ReadAll,
    ])]);
    let mut stream = open(&server).await;
    assert_eq!(raw(stream.next().await.unwrap()).data, UPDATE_DATA);
    let seen = server.wait_for_frames(0, 3).await;
    assert_eq!(seen.frames[2], Received::Pong(b"p1".to_vec()));
}

// real-time: liveness waits run on a real socket.
#[tokio::test]
async fn a_quiet_healthy_stream_stays_open() {
    let interval = Duration::from_millis(500);
    let server = WsServer::start(vec![opened(vec![
        Act::ReadFor(interval * 3),
        Act::Send(Message::text(UPDATE)),
    ])]);
    let mut stream = quick(&server, interval).trade_updates().await.unwrap();
    assert_eq!(raw(stream.next().await.unwrap()).data, UPDATE_DATA);
    let pings = server.seen()[0]
        .frames
        .iter()
        .filter(|frame| matches!(frame, Received::Ping(_)))
        .count();
    assert!(pings >= 2, "{:?}", server.seen());
}

// real-time: as above.
#[tokio::test]
async fn an_unanswered_ping_times_out() {
    let interval = Duration::from_millis(500);
    let server = WsServer::start(vec![opened(vec![Act::Pause(interval * 10)])]);
    let mut stream = quick(&server, interval).trade_updates().await.unwrap();
    let started = tokio::time::Instant::now();
    let error = stream.next().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
    assert!(started.elapsed() < interval * 4, "{:?}", started.elapsed());
    assert!(error.to_string().contains("later messages may be lost"));
}

// real-time: as above.
#[tokio::test]
async fn a_caller_returning_after_a_long_gap_gets_the_message() {
    let interval = Duration::from_millis(300);
    // The server sends its message only after it has read a Ping, so the
    // message cannot already be waiting when the caller returns.
    let server = WsServer::start(vec![opened(vec![
        Act::ReadUntilPing,
        Act::Send(Message::text(UPDATE)),
    ])]);
    let mut stream = quick(&server, interval).trade_updates().await.unwrap();
    tokio::time::sleep(interval * 4).await;
    assert_eq!(raw(stream.next().await.unwrap()).data, UPDATE_DATA);
}

// real-time: as above.
#[tokio::test]
async fn cancelling_next_leaves_the_liveness_clock_alone() {
    let interval = Duration::from_millis(350);
    let server = WsServer::start(vec![opened(vec![Act::Pause(interval * 20)])]);
    let mut stream = quick(&server, interval).trade_updates().await.unwrap();
    let started = tokio::time::Instant::now();
    let error = loop {
        match tokio::time::timeout(interval / 7, stream.next()).await {
            Ok(result) => break result.unwrap_err(),
            Err(_) => continue,
        }
    };
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
    assert!(started.elapsed() < interval * 4, "{:?}", started.elapsed());
}

#[tokio::test]
async fn a_cancelled_next_loses_no_message() {
    let server = WsServer::start(vec![opened(vec![
        Act::Pause(Duration::from_millis(200)),
        Act::Send(Message::text(UPDATE)),
    ])]);
    let mut stream = open(&server).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(50), stream.next())
            .await
            .is_err()
    );
    assert_eq!(raw(stream.next().await.unwrap()).data, UPDATE_DATA);
}

#[tokio::test]
async fn a_cancelled_next_keeps_a_close_whose_reply_is_waiting() {
    // The server sends Pings without reading, so the Pongs fill the
    // connection and the reply to its Close cannot be flushed at once.
    let mut acts: Vec<Act> = (0..100_000)
        .map(|_| Act::Send(Message::Ping(vec![7; 125].into())))
        .collect();
    acts.push(Act::Close(1008, "policy".to_owned()));
    acts.push(Act::Pause(Duration::from_secs(2)));
    acts.push(Act::Drop);
    let server = WsServer::start(vec![opened(acts)]);
    let mut stream = open(&server).await;
    let mut cancelled = 0;
    let error = loop {
        match tokio::time::timeout(Duration::from_millis(20), stream.next()).await {
            Ok(result) => break result.unwrap_err(),
            Err(_) => cancelled += 1,
        }
    };
    assert!(cancelled > 0);
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), Some(1008));
    assert_eq!(error.message(), Some("policy"));
}

#[tokio::test]
async fn close_after_a_cancelled_next_reports_alpacas_close() {
    // As above, but the caller gives up on `next` and closes instead.
    let mut acts: Vec<Act> = (0..100_000)
        .map(|_| Act::Send(Message::Ping(vec![7; 125].into())))
        .collect();
    acts.push(Act::Close(1008, "policy".to_owned()));
    acts.push(Act::Pause(Duration::from_secs(6)));
    acts.push(Act::ReadAll);
    let server = WsServer::start(vec![opened(acts)]);
    let mut stream = open(&server).await;
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        let result = tokio::time::timeout(Duration::from_millis(20), stream.next()).await;
        assert!(result.is_err(), "next returned before the caller gave up");
    }
    let (messages, result) = stream.close().await;
    assert!(messages.is_empty());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), Some(1008));
    assert_eq!(error.message(), Some("policy"));
    // The reply to Alpaca's Close went out, after the queued Pongs, before
    // the connection was dropped.
    let reply = Received::Close(Some((1008, "policy".to_owned())));
    for _ in 0..500 {
        if server.seen()[0].frames.last() == Some(&reply) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the server never read the reply Close");
}

#[tokio::test]
async fn close_returns_what_arrived_while_closing() {
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::text(UPDATE)),
        Act::ReadAll,
    ])]);
    let stream = open(&server).await;
    let (messages, result) = stream.close().await;
    result.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(raw(messages[0].clone()).data, UPDATE_DATA);
    let seen = server.wait_for_frames(0, 3).await;
    assert_eq!(seen.frames[2], Received::Close(Some((1000, String::new()))));
}

#[tokio::test]
async fn close_keeps_its_messages_when_the_connection_fails() {
    let server = WsServer::start(vec![opened(vec![
        Act::Send(Message::text(UPDATE)),
        Act::Pause(Duration::from_millis(100)),
        Act::Drop,
    ])]);
    let stream = open(&server).await;
    tokio::time::sleep(Duration::from_millis(50)).await;
    let (messages, result) = stream.close().await;
    assert_eq!(messages.len(), 1);
    let error = result.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Transport);
    assert!(error.to_string().contains("later messages may be lost"));
}

// real-time: the close's bound runs on a real socket.
#[tokio::test]
async fn an_unanswered_close_times_out() {
    let server = WsServer::start(vec![opened(vec![Act::Pause(Duration::from_secs(5))])]);
    let stream = quick(&server, Duration::from_secs(1))
        .trade_updates()
        .await
        .unwrap();
    let (messages, result) = stream.close().await;
    assert!(messages.is_empty());
    let error = result.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
    assert_eq!(error.to_string(), "the server did not answer the close");
}

#[tokio::test]
async fn close_after_an_end_is_ended() {
    let server = WsServer::start(vec![opened(vec![Act::Drop])]);
    let mut stream = open(&server).await;
    stream.next().await.unwrap_err();
    let (messages, result) = stream.close().await;
    assert!(messages.is_empty());
    assert_eq!(result.unwrap_err().kind(), StreamErrorKind::Ended);
}

#[tokio::test]
async fn a_tls_stream_uses_suris_connector() {
    // An https base URL gives wss://; the listener closes the TCP connection,
    // so the TLS handshake fails. With rustls' two providers on in this
    // build, tokio-tungstenite's own connector would panic instead.
    let server = WsServer::start(vec![Step::CloseTcp]);
    let url = server.url.replace("http://", "https://");
    let client = Client::new(
        Config::new(
            Region::US,
            Environment::Paper,
            Credentials::api_key(KEY, SECRET),
        )
        .with_base_url(url),
    )
    .unwrap();
    let error = client.trade_updates().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Transport);
}
