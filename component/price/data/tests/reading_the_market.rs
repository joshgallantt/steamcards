//! The market on steamcommunity.com, against a stand-in for the site: how a
//! set's cards and a card's order book are asked for and read, and what the
//! market's queue does when the market turns a request down. The queue's
//! real pace, minutes and all, is tested on paused time beside it; these run
//! at a quick one over real requests.

use std::{sync::Arc, time::Duration};

use card::CardKind;
use chrono::{TimeDelta, Utc};
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::AppId;
use money::{Currency, Money};
use price::{Lookup, MarketPause, Price, PriceQuote};
use price_data::{MarketClient, MarketPace, SteamMarketClient};
use steam_api::{
    EResult, SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header_regex, method, path, query_param},
};

/// The market's pace, a thousand times over or more. Its pause is long
/// enough to be sure a request made at once lands in it.
fn quick() -> MarketPace {
    MarketPace {
        signed_in: Duration::from_millis(1),
        jitter: Duration::ZERO,
        signed_out: Duration::from_millis(1),
        first_pause: Duration::from_millis(400),
        longest_pause: Duration::from_millis(1_600),
        after_server_error: Duration::from_millis(10),
    }
}

/// Waits until a moment after Steam's pause ends, by its own end.
async fn past(pause: MarketPause) {
    let left = (pause.until - Utc::now()).to_std().unwrap_or_default();
    tokio::time::sleep(left + Duration::from_millis(50)).await;
}

/// The market as a session signed in as the stand-in's account sees it,
/// with the site at `site`, and a wallet in dollars.
fn signed_in(steam: &FakeSteam, site: &MockServer, name: &str) -> SteamMarketClient {
    steam.wallet_in(1);
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
    let session = SteamClient::with_endpoints(store, &DebugLog::off(), endpoints);
    SteamMarketClient::with_pace(Arc::new(session), quick())
}

/// A page of `search/render`: `total` cards in all, these on this page, in
/// dollars.
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

/// A known price's quote.
fn quote(price: &Price) -> PriceQuote {
    match price {
        Price::Known(quote) => quote.clone(),
        other => panic!("not known: {other:?}"),
    }
}

fn dollars(cents: i64) -> Money {
    Money::new(cents, Currency::USD)
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
    let market = signed_in(&steam, &site, "pages");

    let Lookup::Found(cards) = market
        .look_up_set(AppId(620), CardKind::Normal)
        .await
        .unwrap()
    else {
        panic!("found");
    };

    assert_eq!(cards.len(), 12, "both pages");
    assert_eq!(cards[10].name, "Intro", "named as the set names it");
    assert_eq!(cards[10].market_hash_name, "620-Intro (Trading Card)");
    let intro = quote(&cards[10].price);
    assert_eq!(
        (intro.ask, intro.ask_depth),
        (Some(dollars(8)), Some(40)),
        "its lowest listing"
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
    let market = signed_in(&steam, &site, "foils");

    let Lookup::Found(foils) = market
        .look_up_set(AppId(620), CardKind::Foil)
        .await
        .unwrap()
    else {
        panic!("found");
    };

    assert_eq!(foils[0].name, "Chell");
    assert_eq!(foils[0].market_hash_name, "620-Chell (Foil)");
    assert_eq!(quote(&foils[0].price).ask, Some(dollars(29)));
}

#[tokio::test]
async fn a_market_that_doesnt_list_the_cards_says_so() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    search()
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"success":false}"#))
        .mount(&site)
        .await;
    let market = signed_in(&steam, &site, "unlisted");

    let e = market
        .look_up_set(AppId(620), CardKind::Normal)
        .await
        .unwrap_err();

    assert_eq!(e.to_string(), "the market didn't list the cards");
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
    let market = signed_in(&steam, &site, "paused");
    let requests = || async { site.received_requests().await.unwrap().len() };

    let Lookup::Paused(first) = market
        .look_up_set(AppId(620), CardKind::Normal)
        .await
        .unwrap()
    else {
        panic!("paused");
    };
    assert_eq!(first.step, Duration::from_millis(400));
    assert!(matches!(
        market
            .look_up_set(AppId(620), CardKind::Foil)
            .await
            .unwrap(),
        Lookup::Paused(_)
    ));
    assert_eq!(
        requests().await,
        1,
        "no quick retries, and nothing during the pause"
    );

    past(first).await;
    let Lookup::Paused(again) = market
        .look_up_set(AppId(620), CardKind::Normal)
        .await
        .unwrap()
    else {
        panic!("paused");
    };
    assert_eq!(again.step, Duration::from_millis(800), "twice as long");
    assert_eq!(market.pause(), Some(again));
    assert_eq!(requests().await, 2, "one request to see");

    past(again).await;
    assert!(matches!(
        market
            .look_up_set(AppId(620), CardKind::Normal)
            .await
            .unwrap(),
        Lookup::Found(_)
    ));
    assert_eq!(market.pause(), None, "over once a request gets through");
}

#[tokio::test]
async fn without_a_sign_in_the_market_goes_unasked() {
    let steam = FakeSteam::start().await;
    steam.refuse_logon(EResult::TRY_ANOTHER_CM);
    let site = MockServer::start().await;
    let market = signed_in(&steam, &site, "unasked");

    let answer = market
        .look_up_set(AppId(620), CardKind::Normal)
        .await
        .unwrap();

    assert!(
        matches!(&answer, Lookup::Unanswered(why) if why.contains("TryAnotherCM")),
        "{answer:?}"
    );
    assert!(site.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_pause_from_before_a_restart_is_kept() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let market = signed_in(&steam, &site, "kept");
    let pause = MarketPause {
        until: Utc::now() + TimeDelta::minutes(20),
        step: Duration::from_secs(20 * 60),
    };

    market.resume(pause);

    assert!(matches!(
        market.look_up_set(AppId(620), CardKind::Normal).await.unwrap(),
        Lookup::Paused(p) if p.step == pause.step
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
    let market = signed_in(&steam, &site, "server-error");

    let answer = market
        .look_up_set(AppId(620), CardKind::Normal)
        .await
        .unwrap();

    assert_eq!(
        answer,
        Lookup::Unanswered("steamcommunity.com's market said 502 Bad Gateway, twice".into()),
        "no answer: nothing wrong with the cards asked for"
    );
    assert_eq!(site.received_requests().await.unwrap().len(), 2);
    assert_eq!(market.pause(), None, "not a pause");
}
