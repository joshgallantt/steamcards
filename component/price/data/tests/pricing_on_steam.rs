//! The market as Steam and the disk give it, against stand-ins for Steam
//! and the site: prices in the wallet's currency, Steam's pause kept across
//! a restart, and the settings and prices kept on disk. The market's queue
//! runs at a quick pace here; its real one is tested on paused time in
//! steam-api.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::AppId;
use money::{Currency, Money};
use price::{
    Basis, DefaultKeepPricesUpToDateUseCase, DefaultSetGamesToPriceUseCase,
    KeepPricesUpToDateUseCase, Lookup, Price, PriceEventKind, PriceQuote, PriceRepository,
    PriceSettings, QuoteSource, SetGamesToPriceUseCase, SetPrices, Wallet, system_clock,
};
use price_data::{DefaultPriceRepository, FilePriceStore, MarketPace, SteamMarketClient};
use steam_api::{
    EResult, SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

/// What a file holds, as JSON.
fn on_disk(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

/// Where a test keeps its config file and its prices.
struct Disk {
    config: PathBuf,
    prices: PathBuf,
}

impl Disk {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "steamcards-market-data-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let config = dir.join("config.json");
        let file = ConfigFile::open(config.clone()).unwrap();
        file.save_credentials(Credentials {
            refresh_token: token(STEAM_ID, 4_000_000_000),
            account_name: ACCOUNT.into(),
            steam_id: STEAM_ID,
            login_id: 7,
        })
        .unwrap();
        Self {
            config,
            prices: dir.join("prices.json"),
        }
    }

    /// A repository over what's on disk, as steamcards starts it.
    fn market(
        &self,
        steam: &FakeSteam,
        site: &MockServer,
    ) -> (DefaultPriceRepository, Arc<SteamClient>) {
        let file = Arc::new(ConfigFile::open(self.config.clone()).unwrap());
        let mut endpoints = steam.endpoints();
        endpoints.community = site.uri();
        let session = Arc::new(SteamClient::with_endpoints(
            file.clone(),
            &DebugLog::off(),
            endpoints,
        ));
        let store = FilePriceStore::open(file, self.prices.clone());
        let client = SteamMarketClient::with_pace(
            session.clone(),
            MarketPace {
                signed_in: Duration::from_millis(1),
                jitter: Duration::ZERO,
                signed_out: Duration::from_millis(1),
                first_pause: Duration::from_secs(20 * 60),
                longest_pause: Duration::from_secs(60 * 60),
                after_server_error: Duration::from_millis(10),
            },
        );
        (
            DefaultPriceRepository::new(Arc::new(client), Arc::new(store), DebugLog::off()),
            session,
        )
    }
}

/// `search/render`'s answer for one border of a game: `(name, price, text,
/// listings, type)` each.
fn listing(app_id: u32, cards: &[(&str, i64, &str, u32, &str)]) -> ResponseTemplate {
    let results: Vec<_> = cards
        .iter()
        .map(|&(name, price, text, listings, kind)| {
            serde_json::json!({
                "name": name,
                "hash_name": format!("{app_id}-{name}"),
                "sell_listings": listings,
                "sell_price": price,
                "sell_price_text": text,
                "asset_description": { "type": kind },
            })
        })
        .collect();
    ResponseTemplate::new(200).set_body_json(serde_json::json!({
        "success": true,
        "start": 0,
        "pagesize": 10,
        "total_count": cards.len(),
        "results": results,
    }))
}

async fn lists(site: &MockServer, app_id: u32, border: u8, answer: ResponseTemplate) {
    Mock::given(method("GET"))
        .and(path("/market/search/render/"))
        .and(query_param(
            "category_753_Game[]",
            format!("tag_app_{app_id}"),
        ))
        .and(query_param(
            "category_753_cardborder[]",
            format!("tag_cardborder_{border}"),
        ))
        .respond_with(answer)
        .mount(site)
        .await;
}

const NORMAL: &str = "Heavy Rain Trading Card";
const FOIL: &str = "Heavy Rain Foil Trading Card";

fn ask(price: &Price) -> Option<Money> {
    match price {
        Price::Known(quote) => quote.ask,
        _ => None,
    }
}

#[tokio::test]
async fn a_sets_cards_are_priced_in_the_wallets_currency() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(2);
    let site = MockServer::start().await;
    lists(
        &site,
        960_910,
        0,
        listing(
            960_910,
            &[
                ("Madison", 5, "£0.05", 1_204, NORMAL),
                ("Norman", 7, "$0.07", 88, NORMAL),
                ("Scott", 4, "0,04€", 310, NORMAL),
                ("Ethan", 0, "", 0, NORMAL),
                ("Carter (Foil)", 35, "£0.35", 3, FOIL),
            ],
        ),
    )
    .await;
    let disk = Disk::new("currency");
    let (market, _) = disk.market(&steam, &site);

    let Lookup::Found(cards) = market.look_up_set(AppId(960_910), false).await.unwrap() else {
        panic!("not paused");
    };

    let price = |name: &str| {
        cards
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.price.clone())
    };
    assert_eq!(
        ask(&price("Madison").unwrap()),
        Some(Money::new(5, Currency::GBP))
    );
    assert_eq!(
        ask(&price("Norman").unwrap()),
        Some(Money::new(7, Currency::USD)),
        "in dollars: shown so, and never converted"
    );
    assert_eq!(
        ask(&price("Scott").unwrap()),
        Some(Money::new(4, Currency::EUR)),
        "in euros: shown as it came, never counted with pounds"
    );
    assert_eq!(price("Ethan").unwrap(), Price::NoMarket, "nobody selling");
    assert_eq!(price("Carter"), None, "a foil isn't a normal card");
    let Some(Price::Known(madison)) = price("Madison") else {
        panic!("priced");
    };
    assert_eq!(madison.ask_depth, Some(1_204));
    assert_eq!(madison.source, QuoteSource::Search);
    assert_eq!(cards[0].market_hash_name, "960910-Madison");
}

#[tokio::test]
async fn until_steam_says_the_wallets_currency_prices_wait() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    lists(
        &site,
        620,
        1,
        listing(
            620,
            &[(
                "Chell (Foil)",
                29,
                "£0.29",
                64,
                "Portal 2 Foil Trading Card",
            )],
        ),
    )
    .await;
    let disk = Disk::new("no-wallet-yet");
    let (market, _) = disk.market(&steam, &site);

    let looked_up = market.look_up_set(AppId(620), true).await.unwrap();

    assert_eq!(
        looked_up,
        Lookup::Unanswered("Steam hasn't said the wallet's currency yet".into()),
        "a pound price isn't read as dollars"
    );
    assert_eq!(market.wallet(), None, "Steam hasn't said");
    assert!(
        site.received_requests().await.unwrap().is_empty(),
        "nor is the market asked"
    );
}

#[tokio::test]
async fn an_account_without_a_wallet_has_its_prices_shown_as_they_come() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(0);
    let site = MockServer::start().await;
    lists(
        &site,
        620,
        0,
        listing(620, &[("Chell", 6, "£0.06", 40, "Portal 2 Trading Card")]),
    )
    .await;
    let disk = Disk::new("no-wallet");
    let (market, _) = disk.market(&steam, &site);

    let Lookup::Found(cards) = market.look_up_set(AppId(620), false).await.unwrap() else {
        panic!("found");
    };

    assert_eq!(market.wallet(), Some(Wallet::new(Currency::USD)));
    assert_eq!(
        ask(&cards[0].price),
        Some(Money::new(6, Currency::GBP)),
        "in pounds, as it came: never counted with dollars"
    );
}

#[tokio::test]
async fn the_wallet_is_the_one_steam_tells_of_with_valves_fees() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(3);
    let site = MockServer::start().await;
    let disk = Disk::new("wallet");
    let (market, session) = disk.market(&steam, &site);
    assert_eq!(market.wallet(), None, "not signed on yet");

    session.connection().await.unwrap();

    let euros = market.wallet().unwrap();
    assert_eq!(euros, Wallet::new(Currency::EUR));
    assert_eq!((euros.steam_fee, euros.publisher_fee), (500, 1_000));
}

#[tokio::test]
async fn an_order_book_is_priced_as_its_listing_and_its_offer() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/market/orderbook"))
        .and(query_param("qp", r#"[753,"220-G-Man"]"#))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":{"success":true,"data":{"amtMaxBuyOrder":8,"amtMinSellOrder":11,"eCurrency":2,"cBuyOrders":34378,"cSellOrders":2005}}}"#,
        ))
        .mount(&site)
        .await;
    let disk = Disk::new("offers");
    let (market, _) = disk.market(&steam, &site);

    let Lookup::Found(Price::Known(quote)) = market.look_up_offers("220-G-Man").await.unwrap()
    else {
        panic!("an order book");
    };

    assert_eq!(quote.ask, Some(Money::new(11, Currency::GBP)));
    assert_eq!(quote.bid, Some(Money::new(8, Currency::GBP)));
    assert_eq!(quote.source, QuoteSource::OrderBook);
}

#[tokio::test]
async fn steams_pause_is_kept_across_a_restart() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(2);
    let site = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&site)
        .await;
    let disk = Disk::new("pause");
    let (market, _) = disk.market(&steam, &site);

    let Lookup::Paused(pause) = market.look_up_set(AppId(620), false).await.unwrap() else {
        panic!("paused");
    };
    assert_eq!(pause.step, Duration::from_secs(20 * 60));
    let kept = &on_disk(&disk.config)["market"]["pause"];
    assert_eq!(kept["until"], pause.until.timestamp());
    assert_eq!(kept["step"], 20 * 60);

    // steamcards starts again.
    let (again, _) = disk.market(&steam, &site);
    let Lookup::Paused(still) = again.look_up_offers("620-Chell").await.unwrap() else {
        panic!("still paused");
    };
    assert_eq!(still.until.timestamp(), pause.until.timestamp());
    assert_eq!(
        site.received_requests().await.unwrap().len(),
        1,
        "nothing asked during the pause, before or after the restart"
    );
}

#[tokio::test]
async fn a_market_that_cant_be_asked_marks_no_price_failed() {
    let steam = FakeSteam::start().await;
    steam.refuse_logon(EResult::TRY_ANOTHER_CM);
    let site = MockServer::start().await;
    let disk = Disk::new("cant-ask");
    let (market, _) = disk.market(&steam, &site);
    let market = Arc::new(market);
    DefaultSetGamesToPriceUseCase::new(market.clone())
        .call([960_910, 1_145_360, 620].map(AppId).to_vec());
    let (tx, mut rx) = mpsc::channel(16);
    let token = CancellationToken::new();

    let watching = DefaultKeepPricesUpToDateUseCase::new(market.clone(), system_clock())
        .call(token.clone(), tx);
    let event = tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .unwrap()
        .unwrap();
    token.cancel();
    watching.await.unwrap();

    assert!(
        !matches!(event.kind, PriceEventKind::Failed(_)),
        "{}",
        event.message
    );
    assert!(
        market.book().sets.is_empty(),
        "nothing to show as failed for a day"
    );
    assert!(site.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn the_basis_is_kept_in_the_config_file() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let disk = Disk::new("basis");
    let (market, _) = disk.market(&steam, &site);
    assert_eq!(market.settings().basis, Basis::List, "the default");

    market
        .save_settings(PriceSettings { basis: Basis::Net })
        .unwrap();

    let (again, _) = disk.market(&steam, &site);
    assert_eq!(again.settings().basis, Basis::Net);
    assert_eq!(on_disk(&disk.config)["market"]["basis"], "net");
}

#[tokio::test]
async fn prices_are_kept_for_a_week_across_restarts() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let disk = Disk::new("cache");
    let (market, _) = disk.market(&steam, &site);
    let now = chrono::Utc::now();
    let set = |app_id: u32, days_old: i64| {
        let at = now - chrono::TimeDelta::days(days_old);
        SetPrices {
            app_id: AppId(app_id),
            normal: vec![price::PricedCard {
                name: "Madison".into(),
                market_hash_name: format!("{app_id}-Madison"),
                price: Price::Known(PriceQuote {
                    ask: Some(Money::new(5, Currency::GBP)),
                    bid: None,
                    ask_depth: Some(1_204),
                    bid_depth: None,
                    source: QuoteSource::Search,
                    fetched_at: at,
                }),
            }],
            foil: Vec::new(),
            fetched_at: at,
            retry_at: None,
        }
    };

    market.keep_set(set(960_910, 0));
    market.keep_set(set(1_145_360, 6));
    market.keep_set(set(620, 8));

    let (again, _) = disk.market(&steam, &site);
    let kept: Vec<AppId> = again.book().sets.keys().copied().collect();
    assert_eq!(
        kept,
        [960_910, 1_145_360].map(AppId),
        "a set over a week old goes"
    );
    let madison = &again.book().sets[&AppId(960_910)].normal[0];
    assert_eq!(ask(&madison.price), Some(Money::new(5, Currency::GBP)));
}
