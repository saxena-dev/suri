//! The market-data streams' messages: every kind read from the documented
//! example, field by field, a batch in one frame, the news session as
//! documented, and every double read correctly rounded.

#![cfg(feature = "stream")]

#[path = "support/ws_server.rs"]
mod ws_server;

use suri::data::crypto::{self, Location, TakerSide};
use suri::data::stocks::{self, CancelAction, StreamFeed, Tape};
use suri::data::{Channels, DataMessage, DataStream, news};
use suri::{Client, Config, Credentials, Environment, Inbound, Region, Timestamp};
use tokio_tungstenite::tungstenite::Message;
use ws_server::{Act, WsServer, bounded, data_opened};

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
            Credentials::api_key("PKSENTINELKEY0001", "SECRETSENTINELVALUE0001"),
        )
        .with_base_url(&server.url),
    )
    .unwrap()
}

fn at(text: &str) -> Timestamp {
    text.parse().unwrap()
}

fn tape(code: Tape) -> Inbound<Tape> {
    Inbound::Known(code)
}

/// The messages of one frame of each element, read from a stream.
async fn read<M: DataMessage>(
    elements: &[String],
    open: impl AsyncFnOnce(&Client) -> DataStream<M>,
) -> Vec<M> {
    let acts = elements
        .iter()
        .map(|element| Act::Send(Message::text(format!("[{element}]"))))
        .collect();
    let server = WsServer::start(vec![data_opened(acts)]);
    let mut stream = open(&client(&server)).await;
    let mut messages = Vec::new();
    for _ in elements {
        messages.extend(stream.next().await.unwrap());
    }
    messages
}

async fn stock(elements: &[String]) -> Vec<stocks::Message> {
    read(elements, async |client: &Client| {
        client.stock_stream(StreamFeed::Sip).await.unwrap()
    })
    .await
}

async fn crypto(elements: &[String]) -> Vec<crypto::Message> {
    read(elements, async |client: &Client| {
        client.crypto_stream(Location::Us).await.unwrap()
    })
    .await
}

// The documented examples, as sent.
const TRADE: &str = r#"{"T":"t","i":96921,"S":"AAPL","x":"D","p":126.55,"s":1,"t":"2021-02-22T15:51:44.208Z","c":["@","I"],"z":"C"}"#;
const QUOTE: &str = r#"{"T":"q","S":"AMD","bx":"U","bp":87.66,"bs":1,"ax":"Q","ap":87.68,"as":4,"t":"2021-02-22T15:51:45.335689322Z","c":["R"],"z":"C"}"#;
const BAR: &str = r#"{"T":"b","S":"SPY","o":388.985,"h":389.13,"l":388.975,"c":389.12,"v":49378,"n":461,"vw":389.062639,"t":"2021-02-22T19:15:00Z"}"#;
const CORRECTION: &str = r#"{"T":"c","S":"EEM","x":"M","oi":52983525033527,"op":39.1582,"os":440000,"oc":[" ","7"],"ci":52983525034326,"cp":39.1809,"cs":440000,"cc":[" ","7"],"z":"B","t":"2023-04-06T14:25:06.542305024Z"}"#;
const CANCEL: &str = r#"{"T":"x","S":"GOOGL","i":465,"x":"D","p":105.31,"s":300,"a":"C","z":"C","t":"2023-04-06T13:15:42.83540958Z"}"#;
const LULD: &str = r#"{"T":"l","S":"IONM","u":3.24,"d":2.65,"i":"B","t":"2023-04-06T13:34:45.565004401Z","z":"C"}"#;
const STATUS: &str = r#"{"T":"s","S":"AAPL","sc":"H","sm":"Trading Halt","rc":"T12","rm":"Trading Halted; For information requested by NASDAQ","t":"2021-02-22T19:15:00Z","z":"C"}"#;
const IMBALANCE: &str =
    r#"{"T":"i","S":"INAQU","p":9.12,"z":"C","t":"2024-12-13T19:58:09.242138635Z"}"#;

fn assert_stock_bar(bar: &suri::data::Bar) {
    assert_eq!(bar.timestamp, at("2021-02-22T19:15:00Z"));
    assert_eq!(bar.open, 388.985);
    assert_eq!(bar.high, 389.13);
    assert_eq!(bar.low, 388.975);
    assert_eq!(bar.close, 389.12);
    assert_eq!(bar.volume, 49378);
    assert_eq!(bar.trade_count, 461);
    assert_eq!(bar.vwap, 389.062639);
}

#[tokio::test]
async fn every_stock_message_reads_from_its_documented_example() {
    bounded(async {
        // Daily and updated bars have no example of their own: the bar's, with
        // its type changed.
        let elements: Vec<String> = [
            TRADE.to_owned(),
            QUOTE.to_owned(),
            BAR.to_owned(),
            BAR.replace(r#""T":"b""#, r#""T":"d""#),
            BAR.replace(r#""T":"b""#, r#""T":"u""#),
            CORRECTION.to_owned(),
            CANCEL.to_owned(),
            LULD.to_owned(),
            STATUS.to_owned(),
            IMBALANCE.to_owned(),
        ]
        .into();
        let messages = stock(&elements).await;
        assert_eq!(messages.len(), 10);
        let mut messages = messages.into_iter();

        let Some(stocks::Message::Trade { symbol, trade }) = messages.next() else {
            panic!("not a trade");
        };
        assert_eq!(symbol, "AAPL");
        assert_eq!(trade.id, 96921);
        assert_eq!(trade.exchange, "D");
        assert_eq!(trade.price, 126.55);
        assert_eq!(trade.size, 1);
        assert_eq!(trade.timestamp, at("2021-02-22T15:51:44.208Z"));
        assert_eq!(trade.conditions, ["@", "I"]);
        assert_eq!(trade.tape, tape(Tape::C));
        assert_eq!(trade.update, None);

        let Some(stocks::Message::Quote { symbol, quote }) = messages.next() else {
            panic!("not a quote");
        };
        assert_eq!(symbol, "AMD");
        assert_eq!(quote.bid_exchange, "U");
        assert_eq!(quote.bid_price, 87.66);
        assert_eq!(quote.bid_size, 1);
        assert_eq!(quote.ask_exchange, "Q");
        assert_eq!(quote.ask_price, 87.68);
        assert_eq!(quote.ask_size, 4);
        assert_eq!(quote.timestamp, at("2021-02-22T15:51:45.335689322Z"));
        assert_eq!(quote.conditions, ["R"]);
        assert_eq!(quote.tape, tape(Tape::C));

        let Some(stocks::Message::Bar { symbol, bar }) = messages.next() else {
            panic!("not a bar");
        };
        assert_eq!(symbol, "SPY");
        assert_stock_bar(&bar);
        let Some(stocks::Message::DailyBar { symbol, bar }) = messages.next() else {
            panic!("not a daily bar");
        };
        assert_eq!(symbol, "SPY");
        assert_stock_bar(&bar);
        let Some(stocks::Message::UpdatedBar { symbol, bar }) = messages.next() else {
            panic!("not an updated bar");
        };
        assert_eq!(symbol, "SPY");
        assert_stock_bar(&bar);

        let Some(stocks::Message::Correction { symbol, correction }) = messages.next() else {
            panic!("not a correction");
        };
        assert_eq!(symbol, "EEM");
        assert_eq!(correction.exchange, "M");
        assert_eq!(correction.original_id, 52983525033527);
        assert_eq!(correction.original_price, 39.1582);
        assert_eq!(correction.original_size, 440000);
        assert_eq!(correction.original_conditions, [" ", "7"]);
        assert_eq!(correction.corrected_id, 52983525034326);
        assert_eq!(correction.corrected_price, 39.1809);
        assert_eq!(correction.corrected_size, 440000);
        assert_eq!(correction.corrected_conditions, [" ", "7"]);
        assert_eq!(correction.tape, tape(Tape::B));
        assert_eq!(correction.timestamp, at("2023-04-06T14:25:06.542305024Z"));

        let Some(stocks::Message::Cancel { symbol, cancel }) = messages.next() else {
            panic!("not a cancel");
        };
        assert_eq!(symbol, "GOOGL");
        assert_eq!(cancel.id, 465);
        assert_eq!(cancel.exchange, "D");
        assert_eq!(cancel.price, 105.31);
        assert_eq!(cancel.size, 300);
        assert_eq!(cancel.action, Inbound::Known(CancelAction::Cancel));
        assert_eq!(cancel.tape, tape(Tape::C));
        assert_eq!(cancel.timestamp, at("2023-04-06T13:15:42.83540958Z"));

        let Some(stocks::Message::Luld { symbol, luld }) = messages.next() else {
            panic!("not a luld");
        };
        assert_eq!(symbol, "IONM");
        assert_eq!(luld.limit_up, 3.24);
        assert_eq!(luld.limit_down, 2.65);
        assert_eq!(luld.indicator, "B");
        assert_eq!(luld.timestamp, at("2023-04-06T13:34:45.565004401Z"));
        assert_eq!(luld.tape, tape(Tape::C));

        let Some(stocks::Message::Status { symbol, status }) = messages.next() else {
            panic!("not a status");
        };
        assert_eq!(symbol, "AAPL");
        assert_eq!(status.status_code, "H");
        assert_eq!(status.status_message, "Trading Halt");
        assert_eq!(status.reason_code, "T12");
        assert_eq!(
            status.reason_message,
            "Trading Halted; For information requested by NASDAQ"
        );
        assert_eq!(status.timestamp, at("2021-02-22T19:15:00Z"));
        assert_eq!(status.tape, tape(Tape::C));

        let Some(stocks::Message::Imbalance { symbol, imbalance }) = messages.next() else {
            panic!("not an imbalance");
        };
        assert_eq!(symbol, "INAQU");
        assert_eq!(imbalance.price, 9.12);
        assert_eq!(imbalance.tape, tape(Tape::C));
        assert_eq!(imbalance.timestamp, at("2024-12-13T19:58:09.242138635Z"));
    })
    .await
}

#[tokio::test]
async fn an_error_action_is_kept_as_sent() {
    bounded(async {
        let element = CANCEL.replace(r#""a":"C""#, r#""a":"E""#);
        let messages = stock(&[element, CANCEL.replace(r#""a":"C""#, r#""a":"Z""#)]).await;
        let [
            stocks::Message::Cancel { cancel: error, .. },
            stocks::Message::Cancel {
                cancel: unknown, ..
            },
        ] = messages.as_slice()
        else {
            panic!("not two cancels: {messages:?}");
        };
        assert_eq!(error.action, Inbound::Known(CancelAction::Error));
        assert_eq!(unknown.action, Inbound::Unknown("Z".to_owned()));
    })
    .await
}

#[tokio::test]
async fn a_batch_is_one_call_in_the_order_sent() {
    bounded(async {
        // The documented batch of three trades.
        let batch = r#"[{"T":"t","S":"AAPL","i":628,"x":"K","p":162.92,"s":3,"c":["@","F","T","I"],"z":"C","t":"2023-04-06T11:54:26.838232225Z"},{"T":"t","S":"AAPL","i":75,"x":"Z","p":162.92,"s":3,"c":["@","F","T","I"],"z":"C","t":"2023-04-06T11:54:26.838562809Z"},{"T":"t","S":"AAPL","i":1465,"x":"P","p":162.91,"s":71,"c":["@","F","T","I"],"z":"C","t":"2023-04-06T11:54:26.83915973Z"}]"#;
        let server = WsServer::start(vec![data_opened(vec![Act::Send(Message::text(batch))])]);
        let mut stream = client(&server).stock_stream(StreamFeed::Sip).await.unwrap();
        let messages = stream.next().await.unwrap();
        let ids: Vec<(u64, &str, f64, u32)> = messages
            .iter()
            .map(|message| match message {
                stocks::Message::Trade { trade, .. } => {
                    (trade.id, trade.exchange.as_str(), trade.price, trade.size)
                }
                other => panic!("not a trade: {other:?}"),
            })
            .collect();
        assert_eq!(
            ids,
            [
                (628, "K", 162.92, 3),
                (75, "Z", 162.92, 3),
                (1465, "P", 162.91, 71)
            ]
        );
    })
    .await
}

// The documented crypto examples, as sent.
const CRYPTO_TRADE: &str = r#"{"T":"t","S":"AVAX/USD","p":47.299,"s":29.205707815,"t":"2024-03-12T10:27:48.858228144Z","i":3447222699101865076,"tks":"S"}"#;
const CRYPTO_QUOTE: &str = r#"{"T":"q","S":"BAT/USD","bp":0.35718,"bs":13445.46,"ap":0.3581,"as":13561.902,"t":"2024-03-12T10:29:43.111588173Z"}"#;
const CRYPTO_BAR: &str = r#"{"T":"b","S":"BTC/USD","o":71856.1435,"h":71856.1435,"l":71856.1435,"c":71856.1435,"v":0,"t":"2024-03-12T10:37:00Z","n":0,"vw":0}"#;
const BOOK: &str = r#"{"T":"o","S":"BTC/USD","t":"2024-03-12T10:38:50.79613221Z","b":[{"p":71859.53,"s":0.27994},{"p":71849.4,"s":0.553986},{"p":71820.469,"s":0.83495}],"a":[{"p":71939.7,"s":0.83953},{"p":71940.4,"s":0.28025},{"p":71950.715,"s":0.555928}],"r":true}"#;
const BOOK_UPDATE: &str = r#"{"T":"o","S":"MKR/USD","t":"2024-03-12T10:39:39.445492807Z","b":[],"a":[{"p":2614.587,"s":12.5308}]}"#;
const BOOK_REMOVE: &str = r#"{"T":"o","S":"CRV/USD","t":"2024-03-12T10:39:40.501160019Z","b":[{"p":0.7904,"s":0}],"a":[]}"#;

fn levels(levels: &[crypto::OrderbookLevel]) -> Vec<(f64, f64)> {
    levels
        .iter()
        .map(|level| (level.price, level.size))
        .collect()
}

#[tokio::test]
async fn every_crypto_message_reads_from_its_documented_example() {
    bounded(async {
        let elements: Vec<String> = [
            CRYPTO_TRADE.to_owned(),
            CRYPTO_QUOTE.to_owned(),
            CRYPTO_BAR.to_owned(),
            CRYPTO_BAR.replace(r#""T":"b""#, r#""T":"d""#),
            CRYPTO_BAR.replace(r#""T":"b""#, r#""T":"u""#),
            BOOK.to_owned(),
            BOOK_UPDATE.to_owned(),
            BOOK_REMOVE.to_owned(),
        ]
        .into();
        let mut messages = crypto(&elements).await.into_iter();

        let Some(crypto::Message::Trade { symbol, trade }) = messages.next() else {
            panic!("not a trade");
        };
        assert_eq!(symbol, "AVAX/USD");
        assert_eq!(trade.price, 47.299);
        assert_eq!(trade.size, 29.205707815);
        assert_eq!(trade.timestamp, at("2024-03-12T10:27:48.858228144Z"));
        assert_eq!(trade.id, 3447222699101865076);
        assert_eq!(trade.taker_side, Inbound::Known(TakerSide::Seller));

        let Some(crypto::Message::Quote { symbol, quote }) = messages.next() else {
            panic!("not a quote");
        };
        assert_eq!(symbol, "BAT/USD");
        assert_eq!(quote.bid_price, 0.35718);
        assert_eq!(quote.bid_size, 13445.46);
        assert_eq!(quote.ask_price, 0.3581);
        assert_eq!(quote.ask_size, 13561.902);
        assert_eq!(quote.timestamp, at("2024-03-12T10:29:43.111588173Z"));

        for kind in ["bar", "daily bar", "updated bar"] {
            let bar = match messages.next() {
                Some(crypto::Message::Bar { symbol, bar }) if kind == "bar" => (symbol, bar),
                Some(crypto::Message::DailyBar { symbol, bar }) if kind == "daily bar" => {
                    (symbol, bar)
                }
                Some(crypto::Message::UpdatedBar { symbol, bar }) if kind == "updated bar" => {
                    (symbol, bar)
                }
                other => panic!("not a {kind}: {other:?}"),
            };
            let (symbol, bar) = bar;
            assert_eq!(symbol, "BTC/USD");
            assert_eq!(bar.open, 71856.1435);
            assert_eq!(bar.high, 71856.1435);
            assert_eq!(bar.low, 71856.1435);
            assert_eq!(bar.close, 71856.1435);
            assert_eq!(bar.volume, 0.0);
            assert_eq!(bar.timestamp, at("2024-03-12T10:37:00Z"));
            assert_eq!(bar.trade_count, 0);
            assert_eq!(bar.vwap, 0.0);
        }

        let Some(crypto::Message::Orderbook { symbol, orderbook }) = messages.next() else {
            panic!("not an orderbook");
        };
        assert_eq!(symbol, "BTC/USD");
        assert_eq!(orderbook.timestamp, at("2024-03-12T10:38:50.79613221Z"));
        assert_eq!(
            levels(&orderbook.bids),
            [
                (71859.53, 0.27994),
                (71849.4, 0.553986),
                (71820.469, 0.83495)
            ]
        );
        assert_eq!(
            levels(&orderbook.asks),
            [
                (71939.7, 0.83953),
                (71940.4, 0.28025),
                (71950.715, 0.555928)
            ]
        );
        assert!(orderbook.reset);

        let Some(crypto::Message::Orderbook { symbol, orderbook }) = messages.next() else {
            panic!("not an orderbook update");
        };
        assert_eq!(symbol, "MKR/USD");
        assert_eq!(orderbook.timestamp, at("2024-03-12T10:39:39.445492807Z"));
        assert!(orderbook.bids.is_empty());
        assert_eq!(levels(&orderbook.asks), [(2614.587, 12.5308)]);
        assert!(!orderbook.reset);

        // A size of 0 removes the level; it is delivered as sent.
        let Some(crypto::Message::Orderbook { symbol, orderbook }) = messages.next() else {
            panic!("not an orderbook removal");
        };
        assert_eq!(symbol, "CRV/USD");
        assert_eq!(orderbook.timestamp, at("2024-03-12T10:39:40.501160019Z"));
        assert_eq!(levels(&orderbook.bids), [(0.7904, 0.0)]);
        assert!(orderbook.asks.is_empty());
        assert!(!orderbook.reset);
    })
    .await
}

// The documented news article, as sent.
const ARTICLE: &str = r#"{"T":"n","id":40892639,"headline":"VinFast Officially Launches VF 3 Electric Vehicle In The Philippines","summary":"VinFast Auto has officially opened pre-orders for the VF 3 in the Philippines. From September 19 to 30, early customers who reserve the VF 3 will enjoy several attractive incentives and privileges, including a","author":"Benzinga Newsdesk","created_at":"2024-09-17T09:02:44Z","updated_at":"2024-09-17T09:02:45Z","url":"https://www.benzinga.com/news/24/09/40892639/vinfast-officially-launches-vf-3-electric-vehicle-in-the-philippines","content":"\u003cp\u003eVinFast Auto has officially opened pre-orders for the VF 3 in the Philippines.\u003c/p\u003e\u003cp\u003e\u0026nbsp;\u003c/p\u003e\u003cp\u003eFrom September 19 to 30, early customers who reserve the VF 3 will enjoy several attractive incentives and privileges, including a special price of 605,000 pesos (battery subscription) or 705,000 pesos (battery included). After this period, the prices will revert to the MSRP of 645,000 pesos (battery subscription) and 745,000 pesos (battery included).\u003cbr\u003e\u003cbr\u003eAdditionally, early VF 3 customers will have the privilege of choosing from nine striking exterior paint colors, including four base colors and five premium options, free of charge. Premium paint colors will cost an additional 20,000 pesos after this period.\u003cbr\u003e\u003cbr\u003eMoreover, from September 19 to 30, for only 40,000 pesos, early customers can customize their car's paint beyond the nine available colors. This will be the only time VinFast offers this exclusive privilege for the VF 3.\u003cbr\u003e\u003cbr\u003eVinFast is accepting deposits of 5,000 pesos through its official website or at authorized dealerships (refundable under VinFast's terms).\u003cbr\u003e\u0026nbsp;\u003c/p\u003e","symbols":["VFS"],"source":"benzinga"}"#;

#[tokio::test]
async fn the_documented_news_session_reads_as_sent() {
    bounded(async {
        let server = WsServer::start(vec![data_opened(vec![
            Act::Read,
            Act::Send(Message::text(r#"[{"T":"subscription","news":["*"]}]"#)),
            Act::Send(Message::text(format!("[{ARTICLE}]"))),
        ])]);
        let mut stream = client(&server).news_stream().await.unwrap();
        let mut channels = Channels::new();
        channels.news = vec!["*".to_owned()];
        stream.subscribe(&channels).await.unwrap();

        let messages = stream.next().await.unwrap();
        let [news::Message::Subscription(current)] = messages.as_slice() else {
            panic!("not a subscription: {messages:?}");
        };
        assert_eq!(current.news, ["*"]);
        assert!(current.trades.is_empty());

        let messages = stream.next().await.unwrap();
        let [news::Message::News(article)] = messages.as_slice() else {
            panic!("not an article: {messages:?}");
        };
        assert_eq!(article.id, 40892639);
        assert_eq!(
            article.headline,
            "VinFast Officially Launches VF 3 Electric Vehicle In The Philippines"
        );
        assert!(
            article
                .summary
                .starts_with("VinFast Auto has officially opened")
        );
        assert!(article.summary.ends_with("including a"));
        assert_eq!(article.author, "Benzinga Newsdesk");
        assert_eq!(article.created_at, at("2024-09-17T09:02:44Z"));
        assert_eq!(article.updated_at, at("2024-09-17T09:02:45Z"));
        assert_eq!(
            article.url.as_deref(),
            Some(
                "https://www.benzinga.com/news/24/09/40892639/vinfast-officially-launches-vf-3-electric-vehicle-in-the-philippines"
            )
        );
        // The escapes decode to the HTML they stand for.
        assert!(article.content.starts_with(
            "<p>VinFast Auto has officially opened pre-orders for the VF 3 in the Philippines.</p><p>&nbsp;</p>"
        ));
        assert!(
            article
                .content
                .ends_with("(refundable under VinFast's terms).<br>&nbsp;</p>")
        );
        assert_eq!(article.content.len(), 1108);
        assert_eq!(article.symbols, ["VFS"]);
        assert_eq!(article.source, "benzinga");
        // The stream sends no images.
        assert!(article.images.is_empty());

        let seen = server.wait_for_frames(0, 1).await;
        assert_eq!(seen.texts(), [r#"{"action":"subscribe","news":["*"]}"#]);
    })
    .await
}

#[tokio::test]
async fn every_stock_stream_double_reads_as_the_nearest_f64() {
    bounded(async {
        for sample in SAMPLES {
            let expected: f64 = sample.parse().unwrap();
            let same = |value: f64| assert_eq!(value.to_bits(), expected.to_bits(), "{sample}");
            let elements = [
                format!(
                    r#"{{"T":"c","S":"A","x":"M","oi":1,"op":{sample},"os":1,"oc":[],"ci":2,"cp":{sample},"cs":1,"cc":[],"z":"B","t":"2023-04-06T14:25:06Z"}}"#
                ),
                format!(
                    r#"{{"T":"x","S":"A","i":1,"x":"D","p":{sample},"s":1,"a":"C","z":"C","t":"2023-04-06T14:25:06Z"}}"#
                ),
                format!(
                    r#"{{"T":"l","S":"A","u":{sample},"d":{sample},"i":"B","t":"2023-04-06T14:25:06Z","z":"C"}}"#
                ),
                format!(r#"{{"T":"i","S":"A","p":{sample},"z":"C","t":"2023-04-06T14:25:06Z"}}"#),
            ];
            for message in stock(&elements).await {
                match message {
                    stocks::Message::Correction { correction, .. } => {
                        same(correction.original_price);
                        same(correction.corrected_price);
                    }
                    stocks::Message::Cancel { cancel, .. } => same(cancel.price),
                    stocks::Message::Luld { luld, .. } => {
                        same(luld.limit_up);
                        same(luld.limit_down);
                    }
                    stocks::Message::Imbalance { imbalance, .. } => same(imbalance.price),
                    other => panic!("{sample}: {other:?}"),
                }
            }
        }
    })
    .await
}
