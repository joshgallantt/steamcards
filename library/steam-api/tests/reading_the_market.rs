//! The market on steamcommunity.com, against a stand-in for the site: how a
//! set's cards and a card's order book are asked for and read, and what the
//! market's queue does when the market turns a request down. The queue's
//! real pace, minutes and all, is tested on paused time beside it; these run
//! at a quick one over real requests.
//!
//! Also the wallet, which Steam tells over the CM connection.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    Session,
    cm::WalletInfo,
    market::{Listed, Market, MarketPace, OrderBook},
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, header_exists, header_regex, method, path, query_param},
};

/// The market's pace, a thousand times over or more.
fn quick() -> MarketPace {
    MarketPace {
        signed_in: Duration::from_millis(1),
        jitter: Duration::ZERO,
        signed_out: Duration::from_millis(1),
        first_pause: Duration::from_millis(200),
        longest_pause: Duration::from_millis(800),
        after_server_error: Duration::from_millis(10),
    }
}

/// A session signed in as the stand-in's account, with the site at `site`.
fn signed_in(steam: &FakeSteam, site: &MockServer, name: &str) -> Session {
    let dir = std::env::temp_dir().join(format!("steamcards-market-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    store
        .save_credentials(Credentials {
            refresh_token: token(STEAM_ID, 4_000_000_000),
            account_name: ACCOUNT.into(),
            steam_id: STEAM_ID,
            login_id: 7,
        })
        .unwrap();
    let mut endpoints = steam.endpoints();
    endpoints.community = site.uri();
    Session::with_endpoints(store, &DebugLog::off(), endpoints).with_market_pace(quick())
}

/// A page of `search/render`: `total` cards in all, these on this page.
fn page(total: u32, start: u32, cards: &[(&str, i64)]) -> ResponseTemplate {
    let results: Vec<_> = cards
        .iter()
        .map(|&(name, price)| {
            serde_json::json!({
                "name": name,
                "hash_name": format!("620-{name}"),
                "sell_listings": 40,
                "sell_price": price,
                "sell_price_text": format!("${}.{:02}", price / 100, price % 100),
                "app_name": "Steam",
                "asset_description": {
                    "appid": 753,
                    "type": if name.contains("(Foil") { "Portal 2 Foil Trading Card" } else { "Portal 2 Trading Card" },
                    "market_name": name,
                    "market_hash_name": format!("620-{name}"),
                },
                "sale_price_text": "$0.01",
            })
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "success": true,
        "start": start,
        "pagesize": 10,
        "total_count": total,
        "searchdata": { "query": "", "total_count": total, "pagesize": 10 },
        "results": results,
    }))
}

fn search() -> wiremock::MockBuilder {
    Mock::given(method("GET"))
        .and(path("/market/search/render/"))
        .and(query_param("norender", "1"))
        .and(query_param("appid", "753"))
        .and(query_param("category_753_Game[]", "tag_app_620"))
        .and(query_param("category_753_item_class[]", "tag_item_class_2"))
        .and(query_param("sort_column", "name"))
        .and(query_param("sort_dir", "asc"))
}

#[tokio::test]
async fn a_sets_cards_are_read_a_page_at_a_time_signed_in() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let first: Vec<(String, i64)> = (0..10).map(|i| (format!("Card {i}"), 6 + i)).collect();
    let first: Vec<(&str, i64)> = first.iter().map(|(n, p)| (n.as_str(), *p)).collect();
    search()
        .and(query_param("category_753_cardborder[]", "tag_cardborder_0"))
        .and(query_param("start", "0"))
        .and(header_regex(
            "cookie",
            &format!("^steamLoginSecure={STEAM_ID}%7C%7C"),
        ))
        .respond_with(page(12, 0, &first))
        .mount(&site)
        .await;
    search()
        .and(query_param("category_753_cardborder[]", "tag_cardborder_0"))
        .and(query_param("start", "10"))
        .respond_with(page(12, 10, &[("Intro (Trading Card)", 8), ("The Lab", 8)]))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "pages");

    let Market::Answer(listed) = session.market_search(620, false).await.unwrap() else {
        panic!("not paused");
    };

    assert_eq!(listed.len(), 12, "both pages");
    assert_eq!(
        listed[10],
        Listed {
            hash_name: "620-Intro (Trading Card)".into(),
            name: "Intro".into(),
            sell_price: 8,
            sell_price_text: "$0.08".into(),
            sell_listings: 40,
            item_type: "Portal 2 Trading Card".into(),
        },
        "named as the set names it"
    );
    assert_eq!(site.received_requests().await.unwrap().len(), 2);
}

#[tokio::test]
async fn foils_are_asked_for_by_their_border() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    search()
        .and(query_param("category_753_cardborder[]", "tag_cardborder_1"))
        .and(query_param("start", "0"))
        .respond_with(page(1, 0, &[("Chell (Foil)", 29)]))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "foils");

    let Market::Answer(listed) = session.market_search(620, true).await.unwrap() else {
        panic!("not paused");
    };

    assert_eq!(listed[0].name, "Chell");
    assert_eq!(listed[0].hash_name, "620-Chell (Foil)");
    assert_eq!(listed[0].item_type, "Portal 2 Foil Trading Card");
}

#[tokio::test]
async fn a_market_that_doesnt_list_the_cards_says_so() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    search()
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"success":false}"#))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "unlisted");

    let e = session.market_search(620, false).await.unwrap_err();

    assert_eq!(e.to_string(), "the market didn't list the cards");
}

#[tokio::test]
async fn order_books_are_read_in_either_shape() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let book = |hash: &str| {
        Mock::given(method("GET"))
            .and(path("/market/orderbook"))
            .and(query_param("q", "Load"))
            .and(query_param("qp", format!(r#"[753,"{hash}"]"#)))
            .and(header("x-valve-request-type", "queryAction"))
    };
    book("620-Chell")
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"success":true,"data":{"amtMaxBuyOrder":4,"amtMinSellOrder":5,"eCurrency":2,"cBuyOrders":41763,"cSellOrders":4662}}}"#,
        ))
        .mount(&site)
        .await;
    book("220-G-Man")
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"success":true,"data":{"amtMaxBuyOrder":8,"amtMinSellOrder":11,"eCurrency":2,"cBuyOrders":34378,"cSellOrders":2005}}"#,
        ))
        .mount(&site)
        .await;
    book("730-SAS")
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"data":{"success":false}}"#))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "books");

    assert_eq!(
        session.order_book("620-Chell").await.unwrap(),
        Market::Answer(OrderBook {
            lowest_ask: 5,
            highest_bid: 4,
            sell_orders: 4662,
            buy_orders: 41763,
            currency: 2,
        }),
        "wrapped in an outer data"
    );
    assert_eq!(
        session.order_book("220-G-Man").await.unwrap(),
        Market::Answer(OrderBook {
            lowest_ask: 11,
            highest_bid: 8,
            sell_orders: 2005,
            buy_orders: 34378,
            currency: 2,
        }),
        "bare"
    );
    assert!(
        session.order_book("730-SAS").await.is_err(),
        "a name it doesn't know"
    );
}

#[tokio::test]
async fn an_order_book_that_comes_as_a_page_signed_in_is_asked_for_signed_out() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/market/orderbook"))
        .and(header_exists("cookie"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/html; charset=UTF-8")
                .set_body_string("<!DOCTYPE html><html><body>Steam Community</body></html>"),
        )
        .mount(&site)
        .await;
    Mock::given(method("GET"))
        .and(path("/market/orderbook"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"success":true,"data":{"amtMaxBuyOrder":3,"amtMinSellOrder":4,"eCurrency":1,"cBuyOrders":2,"cSellOrders":156076}}}"#,
        ))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "fallback");

    let book = session.order_book("730-FBI").await.unwrap();

    assert_eq!(
        book,
        Market::Answer(OrderBook {
            lowest_ask: 4,
            highest_bid: 3,
            sell_orders: 156_076,
            buy_orders: 2,
            currency: 1,
        }),
        "in whichever currency the signed-out answer says"
    );
    let asked = site.received_requests().await.unwrap();
    assert_eq!(asked.len(), 2);
    assert!(asked[0].headers.contains_key("cookie"), "signed in first");
    assert!(!asked[1].headers.contains_key("cookie"), "then signed out");
    assert_eq!(
        asked[1].headers.get("x-valve-request-type").unwrap(),
        "queryAction"
    );
}

#[tokio::test]
async fn a_request_the_market_turns_down_pauses_it_and_the_pause_doubles() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    search()
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(2)
        .mount(&site)
        .await;
    search()
        .respond_with(page(1, 0, &[("Chell", 6)]))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "paused");
    let requests = || async { site.received_requests().await.unwrap().len() };

    let Market::Paused(first) = session.market_search(620, false).await.unwrap() else {
        panic!("paused");
    };
    assert_eq!(first.step, Duration::from_millis(200));
    assert!(matches!(
        session.order_book("620-Chell").await.unwrap(),
        Market::Paused(_)
    ));
    assert_eq!(
        requests().await,
        1,
        "no quick retries, and nothing during the pause"
    );

    tokio::time::sleep(Duration::from_millis(250)).await;
    let Market::Paused(again) = session.market_search(620, false).await.unwrap() else {
        panic!("paused");
    };
    assert_eq!(again.step, Duration::from_millis(400), "twice as long");
    assert_eq!(session.market_pause(), Some(again));
    assert_eq!(requests().await, 2, "one request to see");

    tokio::time::sleep(Duration::from_millis(450)).await;
    assert!(matches!(
        session.market_search(620, false).await.unwrap(),
        Market::Answer(_)
    ));
    assert_eq!(
        session.market_pause(),
        None,
        "over once a request gets through"
    );
}

#[tokio::test]
async fn a_pause_from_before_a_restart_is_kept() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let session = signed_in(&steam, &site, "kept");
    let pause = steam_api::market::MarketPause {
        until: std::time::SystemTime::now() + Duration::from_secs(20 * 60),
        step: Duration::from_secs(20 * 60),
    };

    session.resume_market_pause(pause);

    assert!(matches!(
        session.market_search(620, false).await.unwrap(),
        Market::Paused(p) if p.step == pause.step
    ));
    assert!(site.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_server_error_is_asked_once_more_then_reported() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    search()
        .respond_with(ResponseTemplate::new(502))
        .mount(&site)
        .await;
    let session = signed_in(&steam, &site, "server-error");

    let e = session.market_search(620, false).await.unwrap_err();

    assert_eq!(
        e.to_string(),
        "steamcommunity.com's market said 502 Bad Gateway, twice"
    );
    assert_eq!(site.received_requests().await.unwrap().len(), 2);
    assert_eq!(session.market_pause(), None, "not a pause");
}

#[tokio::test]
async fn the_wallet_is_what_steam_says_as_a_session_signs_on() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(2);
    let site = MockServer::start().await;
    let session = signed_in(&steam, &site, "wallet");
    assert_eq!(session.wallet(), None, "not signed on yet");

    session.connection().await.unwrap();

    let pounds = WalletInfo {
        has_wallet: true,
        currency: 2,
    };
    assert_eq!(session.wallet(), Some(pounds));
    session.disconnect().await;
    assert_eq!(
        session.wallet(),
        Some(pounds),
        "kept when the connection goes"
    );
    session.forget().unwrap();
    assert_eq!(session.wallet(), None, "forgotten with the sign-in");
}
