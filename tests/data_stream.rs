//! The market-data streams: the opening on the wire and its failures,
//! subscriptions, one frame per call, nothing dropped, and the core they
//! share with the trade-updates stream.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use std::time::Duration;

use suri::data::crypto::Location;
use suri::data::stocks::{self, StreamFeed};
use suri::data::{Channels, DataStream, news};
use suri::{Client, Config, Credentials, Environment, Region, StreamErrorKind};
use tokio_tungstenite::tungstenite::Message;
use ws_server::{Act, Received, Step, WsServer, data_opened};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";
const TRADE: &str = r#"{"T":"t","S":"AAPL","i":96921,"x":"D","p":126.55,"s":1,"t":"2021-02-22T15:51:44.208Z","c":["@","I"],"z":"C"}"#;

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

async fn open(server: &WsServer) -> DataStream<stocks::Message> {
    client(server).stock_stream(StreamFeed::Iex).await.unwrap()
}

fn send(text: &str) -> Act {
    Act::Send(Message::text(text.to_owned()))
}

/// A frame holding the elements.
fn frame(elements: &[&str]) -> Act {
    send(&format!("[{}]", elements.join(",")))
}

fn assert_send<T: Send>(_: &T) {}

/// The symbol of a stock trade message.
fn trade_symbol(message: &stocks::Message) -> &str {
    match message {
        stocks::Message::Trade { symbol, .. } => symbol,
        other => panic!("not a trade: {other:?}"),
    }
}

#[test]
fn the_streams_and_their_futures_are_send() {
    fn is_send<T: Send>() {}
    is_send::<DataStream<stocks::Message>>();
    is_send::<DataStream<suri::data::crypto::Message>>();
    is_send::<DataStream<news::Message>>();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = WsServer::start(vec![data_opened(vec![])]);
    runtime.block_on(async {
        let client = client(&server);
        assert_send(&client.stock_stream(StreamFeed::Iex));
        assert_send(&client.crypto_stream(Location::Us));
        assert_send(&client.news_stream());
        let mut stream = open(&server).await;
        let channels = Channels::new();
        assert_send(&stream.subscribe(&channels));
        assert_send(&stream.unsubscribe(&channels));
        assert_send(&stream.next());
        assert_send(&stream.close());
    });
}

#[tokio::test]
async fn each_stream_opens_at_its_documented_path_with_the_key_headers() {
    let feeds = [
        (StreamFeed::Sip, "/v2/sip"),
        (StreamFeed::Iex, "/v2/iex"),
        (StreamFeed::DelayedSip, "/v2/delayed_sip"),
        (StreamFeed::Boats, "/v1beta1/boats"),
        (StreamFeed::Overnight, "/v1beta1/overnight"),
        (StreamFeed::Test, "/v2/test"),
    ];
    let paths: Vec<&str> = feeds
        .iter()
        .map(|(_, path)| *path)
        .chain([
            "/v1beta3/crypto/us",
            "/v1beta3/crypto/us-1",
            "/v1beta1/news",
        ])
        .collect();
    let steps = paths
        .iter()
        .map(|_| data_opened(vec![Act::ReadAll]))
        .collect();
    let server = WsServer::start(steps);
    let client = client(&server);
    for (feed, _) in feeds {
        let stream = client.stock_stream(feed).await.unwrap();
        stream.close().await.1.unwrap();
    }
    for location in [Location::Us, Location::Us1] {
        let stream = client.crypto_stream(location).await.unwrap();
        stream.close().await.1.unwrap();
    }
    let stream = client.news_stream().await.unwrap();
    stream.close().await.1.unwrap();

    let seen = server.seen();
    assert_eq!(seen.len(), paths.len());
    for (seen, path) in seen.iter().zip(&paths) {
        assert_eq!(seen.path, *path);
        assert_eq!(seen.header("apca-api-key-id"), Some(KEY));
        assert_eq!(seen.header("apca-api-secret-key"), Some(SECRET));
        assert_eq!(
            seen.header("user-agent"),
            Some(concat!("suri/", env!("CARGO_PKG_VERSION")))
        );
        assert_eq!(seen.header("sec-websocket-extensions"), None);
        // Nothing is written before a subscription: the headers
        // authenticate, so the caller's Close is the first frame.
        assert_eq!(seen.frames, [Received::Close(Some((1000, String::new())))]);
    }
}

#[tokio::test]
async fn an_auth_failure_is_unauthorized_with_alpacas_code_and_message() {
    let failure = r#"[{"T":"error","code":402,"msg":"auth failed"}]"#;
    let connected = r#"[{"T":"success","msg":"connected"}]"#;
    let server = WsServer::start(vec![
        Step::Upgrade(vec![send(failure)]),
        Step::Upgrade(vec![send(connected), send(failure)]),
    ]);
    for _ in 0..2 {
        let error = open_error(&server).await;
        assert_eq!(error.kind(), StreamErrorKind::Unauthorized);
        assert_eq!(error.code(), Some(402));
        assert_eq!(error.message(), Some("auth failed"));
        assert_eq!(
            error.to_string(),
            "Alpaca rejected the stream's credentials (code 402): auth failed; the stream did not open"
        );
    }
}

async fn open_error(server: &WsServer) -> suri::StreamError {
    client(server)
        .stock_stream(StreamFeed::Iex)
        .await
        .unwrap_err()
}

#[tokio::test]
async fn another_error_ends_the_opening_as_a_server_error() {
    let server = WsServer::start(vec![Step::Upgrade(vec![send(
        r#"[{"T":"error","code":406,"msg":"connection limit exceeded"}]"#,
    )])]);
    let error = open_error(&server).await;
    assert_eq!(error.kind(), StreamErrorKind::Server);
    assert_eq!(error.code(), Some(406));
    assert_eq!(error.message(), Some("connection limit exceeded"));
    assert_eq!(
        error.to_string(),
        "Alpaca reported a stream error (code 406): connection limit exceeded; the stream did not open"
    );
}

#[tokio::test]
async fn an_unexpected_opening_frame_is_a_decode_error() {
    let connected = r#"[{"T":"success","msg":"connected"}]"#;
    let server = WsServer::start(vec![
        Step::Upgrade(vec![send(r#"[{"T":"subscription","trades":[]}]"#)]),
        // The welcome where the authentication belongs.
        Step::Upgrade(vec![send(connected), send(connected)]),
        // Two elements in a control frame.
        Step::Upgrade(vec![send(&format!("[{connected},{connected}]"))]),
    ]);
    for _ in 0..3 {
        let error = open_error(&server).await;
        assert_eq!(error.kind(), StreamErrorKind::Decode);
        assert_eq!(error.code(), None);
    }
}

#[tokio::test]
async fn a_refused_upgrade_a_close_and_silence_end_the_opening() {
    let server = WsServer::start(vec![Step::Reject(403)]);
    let error = open_error(&server).await;
    assert_eq!(error.kind(), StreamErrorKind::Handshake);
    assert_eq!(error.status(), Some(403));

    let server = WsServer::start(vec![Step::Upgrade(vec![
        Act::Close(1008, "policy".to_owned()),
        Act::ReadAll,
    ])]);
    let error = open_error(&server).await;
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), Some(1008));

    // real-time: the opening's bound runs on a real socket.
    let interval = Duration::from_millis(300);
    let server = WsServer::start(vec![Step::Upgrade(vec![Act::Pause(interval * 10)])]);
    let started = tokio::time::Instant::now();
    let error = quick(&server, interval)
        .stock_stream(StreamFeed::Iex)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
    assert!(started.elapsed() < interval * 4, "{:?}", started.elapsed());
}

// The documented subscription reply.
const REPLY: &str = r#"[{"T":"subscription","trades":["AAPL"],"quotes":["AMD","CLDR"],"bars":["*"],"updatedBars":[],"dailyBars":["VOO"],"statuses":["*"],"lulds":[],"corrections":["AAPL"],"cancelErrors":["AAPL"]}]"#;

fn list(symbols: &[&str]) -> Vec<String> {
    symbols.iter().map(|symbol| (*symbol).to_owned()).collect()
}

#[tokio::test]
async fn subscribe_and_unsubscribe_send_the_documented_messages() {
    let server = WsServer::start(vec![data_opened(vec![
        Act::Read,
        send(REPLY),
        Act::Read,
        Act::Read,
        Act::ReadAll,
    ])]);
    let mut stream = open(&server).await;
    let mut channels = Channels::new();
    channels.trades = list(&["AAPL"]);
    channels.quotes = list(&["AMD", "CLDR"]);
    channels.bars = list(&["*"]);
    stream.subscribe(&channels).await.unwrap();

    let messages = stream.next().await.unwrap();
    let [stocks::Message::Subscription(current)] = messages.as_slice() else {
        panic!("not one subscription: {messages:?}");
    };
    assert_eq!(current.trades, ["AAPL"]);
    assert_eq!(current.quotes, ["AMD", "CLDR"]);
    assert_eq!(current.bars, ["*"]);
    assert!(current.updated_bars.is_empty());
    assert_eq!(current.daily_bars, ["VOO"]);
    assert_eq!(current.statuses, ["*"]);
    assert!(current.lulds.is_empty());
    assert_eq!(current.corrections, ["AAPL"]);
    assert_eq!(current.cancel_errors, ["AAPL"]);
    assert!(current.imbalances.is_empty());
    assert!(current.orderbooks.is_empty());
    assert!(current.news.is_empty());

    let mut bars = Channels::new();
    bars.bars = list(&["*"]);
    stream.unsubscribe(&bars).await.unwrap();

    // Every channel, in field order, under its wire name.
    let mut every = Channels::new();
    every.trades = list(&["A"]);
    every.quotes = list(&["B"]);
    every.bars = list(&["C"]);
    every.updated_bars = list(&["D"]);
    every.daily_bars = list(&["E"]);
    every.statuses = list(&["F"]);
    every.lulds = list(&["G"]);
    every.corrections = list(&["H"]);
    every.cancel_errors = list(&["I"]);
    every.imbalances = list(&["J"]);
    every.orderbooks = list(&["K"]);
    every.news = list(&["L"]);
    stream.subscribe(&every).await.unwrap();

    let seen = server.wait_for_frames(0, 3).await;
    assert_eq!(
        seen.texts(),
        [
            r#"{"action":"subscribe","trades":["AAPL"],"quotes":["AMD","CLDR"],"bars":["*"]}"#,
            r#"{"action":"unsubscribe","bars":["*"]}"#,
            r#"{"action":"subscribe","trades":["A"],"quotes":["B"],"bars":["C"],"updatedBars":["D"],"dailyBars":["E"],"statuses":["F"],"lulds":["G"],"corrections":["H"],"cancelErrors":["I"],"imbalances":["J"],"orderbooks":["K"],"news":["L"]}"#,
        ]
    );
}

#[tokio::test]
async fn subscribe_after_an_end_is_ended() {
    let server = WsServer::start(vec![data_opened(vec![Act::Drop])]);
    let mut stream = open(&server).await;
    assert_eq!(
        stream.next().await.unwrap_err().kind(),
        StreamErrorKind::Transport
    );
    let error = stream.subscribe(&Channels::new()).await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Ended);
    assert_eq!(error.to_string(), "the stream has already ended");
}

#[tokio::test]
async fn every_element_reaches_the_caller_in_order() {
    let missing_price =
        r#"{"T":"t","S":"AAPL","i":1,"x":"D","s":1,"t":"2021-02-22T15:51:44.208Z","c":[],"z":"C"}"#;
    let unknown = r#"{"T":"zz","S":"AAPL"}"#;
    let untyped = r#"{"S":"AAPL"}"#;
    let array = r#"["t","AAPL"]"#;
    let numeric_symbol = r#"{"T":"t","S":7}"#;
    let last = TRADE.replace("AAPL", "MSFT");
    let server = WsServer::start(vec![data_opened(vec![
        frame(&[
            TRADE,
            missing_price,
            unknown,
            untyped,
            array,
            numeric_symbol,
            &last,
        ]),
        frame(&[r#"{"T":"error","code":405,"msg":"symbol limit exceeded"}"#]),
        send("[]"),
        frame(&[TRADE]),
    ])]);
    let mut stream = open(&server).await;
    let messages = stream.next().await.unwrap();
    assert_eq!(messages.len(), 7);
    assert_eq!(trade_symbol(&messages[0]), "AAPL");
    assert_eq!(
        messages[1],
        stocks::Message::Undecoded(missing_price.to_owned())
    );
    assert_eq!(messages[2], stocks::Message::Other(unknown.to_owned()));
    assert_eq!(messages[3], stocks::Message::Other(untyped.to_owned()));
    assert_eq!(messages[4], stocks::Message::Other(array.to_owned()));
    assert_eq!(
        messages[5],
        stocks::Message::Other(numeric_symbol.to_owned())
    );
    assert_eq!(trade_symbol(&messages[6]), "MSFT");

    // An error after the opening is a message; the stream goes on.
    assert_eq!(
        stream.next().await.unwrap(),
        [stocks::Message::Error {
            code: 405,
            message: "symbol limit exceeded".to_owned()
        }]
    );
    // An empty frame is skipped.
    let messages = stream.next().await.unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(trade_symbol(&messages[0]), "AAPL");
}

#[tokio::test]
async fn a_success_and_a_data_message_without_a_symbol_are_kept() {
    let success = r#"{"T":"success","msg":"authenticated"}"#;
    let anonymous =
        r#"{"T":"t","i":1,"x":"D","p":1,"s":1,"t":"2021-02-22T15:51:44.208Z","c":[],"z":"C"}"#;
    let server = WsServer::start(vec![data_opened(vec![frame(&[success, anonymous])])]);
    let mut stream = open(&server).await;
    assert_eq!(
        stream.next().await.unwrap(),
        [
            stocks::Message::Other(success.to_owned()),
            stocks::Message::Undecoded(anonymous.to_owned()),
        ]
    );
}

#[tokio::test]
async fn a_binary_frame_is_read_as_json() {
    let server = WsServer::start(vec![data_opened(vec![Act::Send(Message::binary(
        format!("[{TRADE}]").into_bytes(),
    ))])]);
    let mut stream = open(&server).await;
    let messages = stream.next().await.unwrap();
    assert_eq!(trade_symbol(&messages[0]), "AAPL");
}

#[tokio::test]
async fn a_frame_that_is_not_an_array_ends_the_stream() {
    for text in [TRADE.to_owned(), format!("[{TRADE}] []"), "[".to_owned()] {
        let server = WsServer::start(vec![data_opened(vec![send(&text)])]);
        let mut stream = open(&server).await;
        let error = stream.next().await.unwrap_err();
        assert_eq!(error.kind(), StreamErrorKind::Decode, "{text}");
        assert!(error.to_string().contains("later messages may be lost"));
        assert_eq!(
            stream.next().await.unwrap_err().kind(),
            StreamErrorKind::Ended
        );
    }
}

#[tokio::test]
async fn a_server_ping_is_answered() {
    let server = WsServer::start(vec![data_opened(vec![
        Act::Send(Message::Ping(b"p1".to_vec().into())),
        frame(&[TRADE]),
        Act::ReadAll,
    ])]);
    let mut stream = open(&server).await;
    stream.next().await.unwrap();
    let seen = server.wait_for_frames(0, 1).await;
    assert_eq!(seen.frames[0], Received::Pong(b"p1".to_vec()));
}

// real-time: liveness waits run on a real socket.
#[tokio::test]
async fn a_silent_stream_is_pinged_then_times_out() {
    let interval = Duration::from_millis(400);
    let server = WsServer::start(vec![data_opened(vec![
        Act::ReadUntilPing,
        Act::Pause(interval * 10),
    ])]);
    let mut stream = quick(&server, interval)
        .stock_stream(StreamFeed::Iex)
        .await
        .unwrap();
    let started = tokio::time::Instant::now();
    let error = tokio::time::timeout(interval * 8, stream.next())
        .await
        .expect("the liveness bound ends the wait")
        .unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Timeout);
    assert!(started.elapsed() < interval * 4, "{:?}", started.elapsed());
    assert!(matches!(server.seen()[0].frames[..], [Received::Ping(_)]));
}

#[tokio::test]
async fn a_server_close_is_closed_and_scrubbed() {
    let reason = format!("bad {SECRET}");
    let server = WsServer::start(vec![data_opened(vec![
        Act::Close(1008, reason),
        Act::ReadAll,
    ])]);
    let mut stream = open(&server).await;
    let error = stream.next().await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), Some(1008));
    assert_eq!(error.message(), Some("bad [REDACTED]"));
    for text in [error.to_string(), format!("{error:?}")] {
        assert!(!text.contains(SECRET) && !text.contains(KEY), "{text}");
    }
}

#[tokio::test]
async fn a_close_read_by_a_cancelled_next_is_kept() {
    // The server sends Pings without reading, so the Pongs fill the
    // connection and the reply to its Close cannot be flushed at once.
    let mut acts: Vec<Act> = (0..100_000)
        .map(|_| Act::Send(Message::Ping(vec![7; 125].into())))
        .collect();
    acts.push(Act::Close(1008, "policy".to_owned()));
    acts.push(Act::Pause(Duration::from_secs(2)));
    acts.push(Act::Drop);
    let server = WsServer::start(vec![data_opened(acts)]);
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
}

#[tokio::test]
async fn subscribe_after_a_cancelled_next_reports_alpacas_close() {
    // As above, but the caller gives up on `next` and subscribes instead:
    // the kept Close is returned and nothing is written.
    let mut acts: Vec<Act> = (0..100_000)
        .map(|_| Act::Send(Message::Ping(vec![7; 125].into())))
        .collect();
    acts.push(Act::Close(1008, "policy".to_owned()));
    acts.push(Act::Pause(Duration::from_secs(6)));
    acts.push(Act::ReadAll);
    let server = WsServer::start(vec![data_opened(acts)]);
    let mut stream = open(&server).await;
    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_secs(2) {
        let result = tokio::time::timeout(Duration::from_millis(20), stream.next()).await;
        assert!(result.is_err(), "next returned before the caller gave up");
    }
    let mut channels = Channels::new();
    channels.trades = list(&["AAPL"]);
    let error = stream.subscribe(&channels).await.unwrap_err();
    assert_eq!(error.kind(), StreamErrorKind::Closed);
    assert_eq!(error.close_code(), Some(1008));
    assert_eq!(
        stream.next().await.unwrap_err().kind(),
        StreamErrorKind::Ended
    );
    assert!(server.seen()[0].texts().is_empty());
}

#[tokio::test]
async fn close_returns_the_messages_of_frames_read_while_closing() {
    let server = WsServer::start(vec![data_opened(vec![
        frame(&[TRADE, TRADE]),
        frame(&[TRADE]),
        Act::ReadAll,
    ])]);
    let stream = open(&server).await;
    let (messages, result) = stream.close().await;
    result.unwrap();
    assert_eq!(messages.len(), 3);
    for message in &messages {
        assert_eq!(trade_symbol(message), "AAPL");
    }
}
