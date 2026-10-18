//! The options stream over MessagePack: the opening and its content type,
//! subscriptions as named maps, each timestamp form, numbers, every element
//! kept, and the frame rules.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use rmp::encode;
use suri::data::options::{Feed, Message as OptionMessage};
use suri::data::{Channels, DataStream};
use suri::{Client, Config, Credentials, Environment, Region, StreamErrorKind, Timestamp};
use tokio_tungstenite::tungstenite::Message;
use ws_server::{Act, Received, Step, WsServer, bounded, msgpack_opened};

const KEY: &str = "PKSENTINELKEY0001";
const SECRET: &str = "SECRETSENTINELVALUE0001";

const SAMPLES: [&str; 4] = [
    "8425.21",
    "0.30000000000000004",
    "1.1368683772161603e-13",
    "9007199254740993",
];

fn client(server: &WsServer) -> Client {
    Client::new(
        Config::new(
            Region::US,
            Environment::Paper,
            Credentials::api_key(KEY, SECRET),
        )
        .with_base_url(&server.url),
    )
    .unwrap()
}

async fn open(server: &WsServer) -> DataStream<OptionMessage> {
    client(server).option_stream(Feed::Opra).await.unwrap()
}

/// A MessagePack value, written with `rmp`'s encoders.
enum Value<'a> {
    Str(&'a str),
    F64(f64),
    F32(f32),
    Uint(u64),
    Int(i64),
    Ext(i8, Vec<u8>),
    Array(Vec<Value<'a>>),
    Map(Vec<(&'a str, Value<'a>)>),
}

fn write(buffer: &mut Vec<u8>, value: &Value<'_>) {
    match value {
        Value::Str(text) => encode::write_str(buffer, text).unwrap(),
        Value::F64(number) => encode::write_f64(buffer, *number).unwrap(),
        Value::F32(number) => encode::write_f32(buffer, *number).unwrap(),
        Value::Uint(number) => drop(encode::write_uint(buffer, *number).unwrap()),
        Value::Int(number) => drop(encode::write_sint(buffer, *number).unwrap()),
        Value::Ext(kind, data) => {
            encode::write_ext_meta(buffer, data.len() as u32, *kind).unwrap();
            buffer.extend(data);
        }
        Value::Array(items) => {
            encode::write_array_len(buffer, items.len() as u32).unwrap();
            for item in items {
                write(buffer, item);
            }
        }
        Value::Map(entries) => {
            encode::write_map_len(buffer, entries.len() as u32).unwrap();
            for (key, item) in entries {
                encode::write_str(buffer, key).unwrap();
                write(buffer, item);
            }
        }
    }
}

fn bytes(value: &Value<'_>) -> Vec<u8> {
    let mut buffer = Vec::new();
    write(&mut buffer, value);
    buffer
}

/// A binary frame holding these elements as one array.
fn frame(elements: Vec<Value<'_>>) -> Act {
    Act::Send(Message::binary(bytes(&Value::Array(elements))))
}

/// The 32-bit timestamp: seconds only.
fn ts32(seconds: u32) -> Value<'static> {
    Value::Ext(-1, seconds.to_be_bytes().to_vec())
}

/// The 64-bit timestamp: nanoseconds above 34 bits of seconds.
fn ts64(seconds: u64, nanoseconds: u32) -> Value<'static> {
    Value::Ext(
        -1,
        ((u64::from(nanoseconds) << 34) | seconds)
            .to_be_bytes()
            .to_vec(),
    )
}

/// The 96-bit timestamp: nanoseconds, then signed seconds.
fn ts96(seconds: i64, nanoseconds: u32) -> Value<'static> {
    let mut data = nanoseconds.to_be_bytes().to_vec();
    data.extend(seconds.to_be_bytes());
    Value::Ext(-1, data)
}

fn trade<'a>(time: Value<'a>, price: Value<'a>) -> Value<'a> {
    Value::Map(vec![
        ("T", Value::Str("t")),
        ("S", Value::Str("AAPL240315C00172500")),
        ("t", time),
        ("p", price),
        ("s", Value::Uint(1)),
        ("x", Value::Str("N")),
        ("c", Value::Str("S")),
    ])
}

fn quote<'a>(time: Value<'a>, bid: Value<'a>, ask: Value<'a>) -> Value<'a> {
    Value::Map(vec![
        ("T", Value::Str("q")),
        ("S", Value::Str("SPXW240327P04925000")),
        ("t", time),
        ("bx", Value::Str("C")),
        ("bp", bid),
        ("bs", Value::Uint(53)),
        ("ax", Value::Str("c")),
        ("ap", ask),
        ("as", Value::Uint(38)),
        ("c", Value::Str("A")),
    ])
}

fn at(seconds: i64, nanoseconds: u32) -> Timestamp {
    Timestamp::from_unix(seconds, nanoseconds).unwrap()
}

#[test]
fn the_stream_and_its_futures_are_send() {
    fn is_send<T: Send>() {}
    fn assert_send<T: Send>(_: &T) {}
    is_send::<DataStream<OptionMessage>>();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let server = WsServer::start(vec![msgpack_opened(vec![])]);
    runtime.block_on(async {
        assert_send(&client(&server).option_stream(Feed::Opra));
        let mut stream = open(&server).await;
        assert_send(&stream.next());
    });
}

#[tokio::test]
async fn the_opening_asks_for_messagepack_at_the_feeds_path() {
    bounded(async {
        let server = WsServer::start(vec![
            msgpack_opened(vec![Act::ReadAll]),
            msgpack_opened(vec![Act::ReadAll]),
        ]);
        for feed in [Feed::Opra, Feed::Indicative] {
            let stream = client(&server).option_stream(feed).await.unwrap();
            stream.close().await.1.unwrap();
        }
        let seen = server.seen();
        for (seen, path) in seen.iter().zip(["/v1beta1/opra", "/v1beta1/indicative"]) {
            assert_eq!(seen.path, path);
            assert_eq!(seen.header("content-type"), Some("application/msgpack"));
            assert_eq!(seen.header("apca-api-key-id"), Some(KEY));
            assert_eq!(seen.header("apca-api-secret-key"), Some(SECRET));
            assert_eq!(seen.frames, [Received::Close(Some((1000, String::new())))]);
        }
    })
    .await
}

#[tokio::test]
async fn a_messagepack_auth_failure_and_a_json_opening_end_the_opening() {
    bounded(async {
        let failure = bytes(&Value::Array(vec![Value::Map(vec![
            ("T", Value::Str("error")),
            ("code", Value::Uint(402)),
            ("msg", Value::Str("auth failed")),
        ])]));
        let server = WsServer::start(vec![
            Step::Upgrade(vec![Act::Send(Message::binary(failure))]),
            // A JSON welcome where MessagePack belongs.
            Step::Upgrade(vec![Act::Send(Message::text(
                r#"[{"T":"success","msg":"connected"}]"#,
            ))]),
        ]);
        let error = client(&server).option_stream(Feed::Opra).await.unwrap_err();
        assert_eq!(error.kind(), StreamErrorKind::Unauthorized);
        assert_eq!(error.code(), Some(402));
        assert_eq!(error.message(), Some("auth failed"));
        let error = client(&server).option_stream(Feed::Opra).await.unwrap_err();
        assert_eq!(error.kind(), StreamErrorKind::Decode);
    })
    .await
}

#[tokio::test]
async fn a_subscription_is_a_named_messagepack_map() {
    bounded(async {
        let server = WsServer::start(vec![msgpack_opened(vec![Act::Read, Act::Read])]);
        let mut stream = open(&server).await;
        let mut channels = Channels::new();
        channels.trades = vec!["AAPL240315C00172500".to_owned()];
        channels.quotes = vec!["SPXW240327P04925000".to_owned()];
        stream.subscribe(&channels).await.unwrap();
        let mut quotes = Channels::new();
        quotes.quotes = vec!["SPXW240327P04925000".to_owned()];
        stream.unsubscribe(&quotes).await.unwrap();

        // Written by hand: a map of three, then of two, with string keys.
        let mut subscribe = vec![0x83, 0xa6];
        subscribe.extend(b"action");
        subscribe.push(0xa9);
        subscribe.extend(b"subscribe");
        subscribe.push(0xa6);
        subscribe.extend(b"trades");
        subscribe.extend([0x91, 0xb3]);
        subscribe.extend(b"AAPL240315C00172500");
        subscribe.push(0xa6);
        subscribe.extend(b"quotes");
        subscribe.extend([0x91, 0xb3]);
        subscribe.extend(b"SPXW240327P04925000");
        let mut unsubscribe = vec![0x82, 0xa6];
        unsubscribe.extend(b"action");
        unsubscribe.push(0xab);
        unsubscribe.extend(b"unsubscribe");
        unsubscribe.push(0xa6);
        unsubscribe.extend(b"quotes");
        unsubscribe.extend([0x91, 0xb3]);
        unsubscribe.extend(b"SPXW240327P04925000");
        let seen = server.wait_for_frames(0, 2).await;
        assert_eq!(
            seen.frames,
            [Received::Binary(subscribe), Received::Binary(unsubscribe)]
        );
    })
    .await
}

#[tokio::test]
async fn each_timestamp_form_and_number_form_reads() {
    bounded(async {
        let server = WsServer::start(vec![msgpack_opened(vec![
            // 32-bit time, float64 prices.
            frame(vec![
                trade(ts32(1_710_164_135), Value::F64(2.84)),
                quote(ts32(1_710_244_778), Value::F64(9.46), Value::F64(9.66)),
            ]),
            // 64-bit time, float32 prices.
            frame(vec![
                trade(ts64(1_710_164_135, 133_122_560), Value::F32(2.5)),
                quote(
                    ts64(1_710_244_778, 897_261_568),
                    Value::F32(9.25),
                    Value::F32(0.1),
                ),
            ]),
            // 96-bit time, integer prices.
            frame(vec![
                trade(ts96(-1, 999_999_999), Value::Uint(3)),
                quote(ts96(1_710_244_778, 1), Value::Uint(9), Value::Int(-1)),
            ]),
            // A JSON string time, as the docs show it.
            frame(vec![trade(
                Value::Str("2024-03-11T13:35:35.13312256Z"),
                Value::F64(2.84),
            )]),
        ])]);
        let mut stream = open(&server).await;
        let expected = [
            (at(1_710_164_135, 0), 2.84, at(1_710_244_778, 0), 9.46, 9.66),
            (
                at(1_710_164_135, 133_122_560),
                2.5,
                at(1_710_244_778, 897_261_568),
                9.25,
                f64::from(0.1_f32),
            ),
            (at(-1, 999_999_999), 3.0, at(1_710_244_778, 1), 9.0, -1.0),
        ];
        for (trade_time, price, quote_time, bid, ask) in expected {
            let messages = stream.next().await.unwrap();
            let [
                OptionMessage::Trade { symbol, trade },
                OptionMessage::Quote {
                    symbol: quoted,
                    quote,
                },
            ] = messages.as_slice()
            else {
                panic!("not a trade and a quote: {messages:?}");
            };
            assert_eq!(symbol, "AAPL240315C00172500");
            assert_eq!(trade.timestamp, trade_time);
            assert_eq!(trade.price, price);
            assert_eq!(trade.size, 1);
            assert_eq!(trade.exchange, "N");
            assert_eq!(trade.condition, "S");
            assert_eq!(quoted, "SPXW240327P04925000");
            assert_eq!(quote.timestamp, quote_time);
            assert_eq!(quote.bid_exchange, "C");
            assert_eq!(quote.bid_price, bid);
            assert_eq!(quote.bid_size, 53);
            // Exchange codes keep their case.
            assert_eq!(quote.ask_exchange, "c");
            assert_eq!(quote.ask_price, ask);
            assert_eq!(quote.ask_size, 38);
            assert_eq!(quote.condition, "A");
        }
        let messages = stream.next().await.unwrap();
        let [OptionMessage::Trade { trade, .. }] = messages.as_slice() else {
            panic!("not a trade: {messages:?}");
        };
        assert_eq!(trade.timestamp, at(1_710_164_135, 133_122_560));
    })
    .await
}

#[tokio::test]
async fn every_double_reads_as_the_nearest_f64() {
    bounded(async {
        let frames = SAMPLES
            .iter()
            .map(|sample| {
                let number = sample.parse::<f64>().unwrap();
                frame(vec![
                    trade(ts32(1), Value::F64(number)),
                    quote(ts32(1), Value::F64(number), Value::F64(number)),
                ])
            })
            .collect();
        let server = WsServer::start(vec![msgpack_opened(frames)]);
        let mut stream = open(&server).await;
        for sample in SAMPLES {
            let expected = sample.parse::<f64>().unwrap().to_bits();
            let messages = stream.next().await.unwrap();
            let [
                OptionMessage::Trade { trade, .. },
                OptionMessage::Quote { quote, .. },
            ] = messages.as_slice()
            else {
                panic!("{sample}: {messages:?}");
            };
            for value in [trade.price, quote.bid_price, quote.ask_price] {
                assert_eq!(value.to_bits(), expected, "{sample}");
            }
        }
    })
    .await
}

#[tokio::test]
async fn every_element_is_kept_as_its_bytes() {
    bounded(async {
        let unknown = Value::Map(vec![("T", Value::Str("zz")), ("S", Value::Str("A"))]);
        let missing_price = Value::Map(vec![
            ("T", Value::Str("t")),
            ("S", Value::Str("A")),
            ("t", ts32(1)),
            ("s", Value::Uint(1)),
            ("x", Value::Str("N")),
            ("c", Value::Str("S")),
        ]);
        let array = Value::Array(vec![Value::Str("t"), Value::Str("A")]);
        let fractional_size = Value::Map(vec![
            ("T", Value::Str("t")),
            ("S", Value::Str("A")),
            ("t", ts32(1)),
            ("p", Value::F64(1.0)),
            ("s", Value::F64(1.5)),
            ("x", Value::Str("N")),
            ("c", Value::Str("S")),
        ]);
        // Extensions that are not timestamps: another type, a length the
        // format does not define, and nanoseconds of a whole second.
        let other_type = trade(Value::Ext(1, vec![0, 0, 0, 1]), Value::F64(1.0));
        let five_bytes = trade(Value::Ext(-1, vec![0, 0, 0, 0, 1]), Value::F64(1.0));
        let whole_second = trade(ts64(1, 1_000_000_000), Value::F64(1.0));
        let error = Value::Map(vec![
            ("T", Value::Str("error")),
            ("code", Value::Uint(413)),
            (
                "msg",
                Value::Str("star subscription is not allowed for option quotes"),
            ),
        ]);
        let expected = [
            OptionMessage::Other(bytes(&unknown)),
            OptionMessage::Undecoded(bytes(&missing_price)),
            OptionMessage::Other(bytes(&array)),
            OptionMessage::Undecoded(bytes(&fractional_size)),
            OptionMessage::Undecoded(bytes(&other_type)),
            OptionMessage::Undecoded(bytes(&five_bytes)),
            OptionMessage::Undecoded(bytes(&whole_second)),
            OptionMessage::Error {
                code: 413,
                message: "star subscription is not allowed for option quotes".to_owned(),
            },
        ];
        let server = WsServer::start(vec![msgpack_opened(vec![
            frame(vec![
                unknown,
                missing_price,
                array,
                fractional_size,
                other_type,
                five_bytes,
                whole_second,
                error,
            ]),
            // An empty array is skipped.
            Act::Send(Message::binary(vec![0x90])),
            frame(vec![Value::Map(vec![
                ("T", Value::Str("subscription")),
                ("trades", Value::Array(vec![Value::Str("A")])),
            ])]),
        ])]);
        let mut stream = open(&server).await;
        assert_eq!(stream.next().await.unwrap(), expected);
        let messages = stream.next().await.unwrap();
        let [OptionMessage::Subscription(current)] = messages.as_slice() else {
            panic!("not a subscription: {messages:?}");
        };
        assert_eq!(current.trades, ["A"]);
    })
    .await
}

#[tokio::test]
async fn a_frame_that_is_not_one_messagepack_array_ends_the_stream() {
    bounded(async {
        let mut trailing = bytes(&Value::Array(vec![trade(ts32(1), Value::F64(1.0))]));
        trailing.push(0xc0);
        let mut cut = bytes(&Value::Array(vec![trade(ts32(1), Value::F64(1.0))]));
        cut.truncate(cut.len() - 3);
        // A header claiming four billion elements over a few bytes.
        let claim = vec![0xdd, 0xff, 0xff, 0xff, 0xff, 0x90];
        let not_array = bytes(&trade(ts32(1), Value::F64(1.0)));
        // Nested 129 levels, one past the limit, and 5,002 levels: errors,
        // not a stack overflow.
        let mut past_limit = vec![0x91; 128];
        past_limit.push(0x90);
        let mut deep = vec![0x91; 5_001];
        deep.push(0x90);
        let frames = [
            Message::binary(trailing),
            Message::binary(cut),
            Message::binary(claim),
            Message::binary(not_array),
            Message::binary(past_limit),
            Message::binary(deep),
            Message::text("[]"),
        ];
        for frame in frames {
            let server = WsServer::start(vec![msgpack_opened(vec![Act::Send(frame.clone())])]);
            let mut stream = open(&server).await;
            // Bounded, so a frame skipped by mistake fails here instead of
            // waiting on a connection that answers every Ping.
            let error = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
                .await
                .expect("a bad frame ends the stream at once")
                .unwrap_err();
            assert_eq!(error.kind(), StreamErrorKind::Decode, "{frame:?}");
            assert_eq!(
                stream.next().await.unwrap_err().kind(),
                StreamErrorKind::Ended
            );
        }
    })
    .await
}

#[tokio::test]
async fn an_element_nested_within_the_limit_is_kept() {
    bounded(async {
        // Exactly 128 levels, the limit: the frame's array, then an element
        // 127 levels deep.
        let mut nested = vec![0x91; 127];
        nested.push(0x90);
        let element = nested[1..].to_vec();
        let server = WsServer::start(vec![msgpack_opened(vec![Act::Send(Message::binary(
            nested,
        ))])]);
        let mut stream = open(&server).await;
        let messages = tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .expect("the frame is read at once")
            .unwrap();
        assert_eq!(messages, [OptionMessage::Other(element)]);
    })
    .await
}

#[tokio::test]
async fn close_returns_the_messages_read_while_closing() {
    bounded(async {
        let server = WsServer::start(vec![msgpack_opened(vec![
            frame(vec![trade(ts32(1), Value::F64(1.0))]),
            Act::ReadAll,
        ])]);
        let stream = open(&server).await;
        let (messages, result) = stream.close().await;
        result.unwrap();
        assert!(matches!(messages[..], [OptionMessage::Trade { .. }]));
    })
    .await
}
