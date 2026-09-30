//! Describing the items in the account's inventory over the CM connection,
//! against a stand-in for Steam: what's asked, and what each item is.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    EResult, SteamClient,
    inventory::InventoryItem,
    test_support::{ACCOUNT, FakeSteam, HeldItem, InventoryAsk, STEAM_ID, token},
};

/// How long before asking Steam again: Steam's own 2 seconds is tested on
/// paused time beside the session; over a real socket, a moment.
const AGAIN: Duration = Duration::from_millis(20);

/// A session signed in as the stand-in's account, with a sign-in good for
/// months.
fn signed_in(steam: &FakeSteam, name: &str) -> SteamClient {
    let dir = std::env::temp_dir().join(format!("steamcards-{name}-{}", std::process::id()));
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
    SteamClient::with_endpoints(store, &DebugLog::off(), steam.endpoints())
        .with_ask_again_after(AGAIN)
}

fn asked_ids(steam: &FakeSteam) -> Vec<Vec<u64>> {
    steam
        .inventory_asks()
        .into_iter()
        .map(|a| a.asset_ids)
        .collect()
}

#[tokio::test]
async fn items_are_asked_for_as_steams_own_site_asks() {
    let steam = FakeSteam::start().await;
    steam.hold(vec![HeldItem::card(11, 620, "Chell")]);
    let session = signed_in(&steam, "ask");

    let items = session.describe_items(&[11, 11]).await.unwrap();

    assert_eq!(items.len(), 1, "each item once");
    assert_eq!(
        steam.inventory_asks(),
        [InventoryAsk {
            steam_id: STEAM_ID,
            app_id: 753,
            context_id: 6,
            descriptions: true,
            language: "english".into(),
            asset_ids: vec![11],
        }],
        "the account's community items, described in English"
    );
}

#[tokio::test]
async fn what_an_item_is_comes_from_its_tags() {
    let steam = FakeSteam::start().await;
    steam.hold(vec![
        HeldItem::card(11, 730, "Anarchist (Trading Card)"),
        HeldItem::foil(12, 620, "Chell (Foil)"),
        HeldItem::other(13, 620, 5, "Portal 2 Booster Pack"),
    ]);
    let session = signed_in(&steam, "tags");

    let items = session.describe_items(&[12, 99, 11, 13]).await.unwrap();

    assert_eq!(
        items,
        [
            InventoryItem {
                asset_id: 12,
                app_id: Some(620),
                trading_card: true,
                foil: true,
                name: "Chell".into(),
                market_hash_name: "620-Chell (Foil)".into(),
                marketable: true,
                tradable: true,
            },
            InventoryItem {
                asset_id: 11,
                app_id: Some(730),
                trading_card: true,
                foil: false,
                name: "Anarchist".into(),
                market_hash_name: "730-Anarchist (Trading Card)".into(),
                marketable: true,
                tradable: true,
            },
            InventoryItem {
                asset_id: 13,
                app_id: Some(620),
                trading_card: false,
                foil: false,
                name: "Portal 2 Booster Pack".into(),
                market_hash_name: "620-Portal 2 Booster Pack".into(),
                marketable: true,
                tradable: true,
            },
        ],
        "in the order asked, and 99 isn't in the inventory"
    );
    assert_eq!(
        asked_ids(&steam),
        [vec![12, 99, 11, 13], vec![99]],
        "Steam doesn't know 99: asked about once more, then left out"
    );
}

#[tokio::test]
async fn an_item_steam_hasnt_caught_up_with_is_asked_about_again() {
    let steam = FakeSteam::start().await;
    steam.hold(vec![
        HeldItem::card(21, 960_910, "Madison"),
        HeldItem::card(22, 960_910, "Madison").late(),
    ]);
    let session = signed_in(&steam, "late");
    let started = Instant::now();

    let items = session.describe_items(&[21, 22]).await.unwrap();

    assert!(started.elapsed() >= AGAIN, "a moment later");
    assert_eq!(asked_ids(&steam), [vec![21, 22], vec![22]]);
    let copies: Vec<(u64, &str)> = items
        .iter()
        .map(|i| (i.asset_id, i.name.as_str()))
        .collect();
    assert_eq!(
        copies,
        [(21, "Madison"), (22, "Madison")],
        "two copies of one card"
    );
}

#[tokio::test]
async fn an_ask_steam_turns_away_is_asked_once_more() {
    let steam = FakeSteam::start().await;
    steam.hold(vec![HeldItem::card(21, 960_910, "Madison")]);
    steam.refuse_inventory(1, EResult::BUSY);
    let session = signed_in(&steam, "busy");

    let items = session.describe_items(&[21]).await.unwrap();

    assert_eq!(items.len(), 1, "Madison, the second time");
    assert_eq!(asked_ids(&steam), [vec![21], vec![21]]);
}

#[tokio::test]
async fn an_ask_steam_keeps_turning_away_says_why() {
    let steam = FakeSteam::start().await;
    steam.refuse_inventory(2, EResult::SERVICE_UNAVAILABLE);
    let session = signed_in(&steam, "unavailable");

    let e = session.describe_items(&[21]).await.unwrap_err();

    assert_eq!(e.to_string(), "Steam said ServiceUnavailable (20)");
    assert_eq!(asked_ids(&steam).len(), 2, "asked twice, and no more");
}

#[tokio::test]
async fn an_ask_steam_refuses_outright_isnt_asked_again() {
    let steam = FakeSteam::start().await;
    steam.refuse_inventory(1, EResult::ACCESS_DENIED);
    let session = signed_in(&steam, "denied");

    assert!(session.describe_items(&[21]).await.is_err());
    assert_eq!(asked_ids(&steam).len(), 1);
}

#[tokio::test]
async fn describing_nothing_asks_steam_nothing() {
    let steam = FakeSteam::start().await;
    steam.hold(vec![HeldItem::card(11, 620, "Chell")]);
    let session = signed_in(&steam, "nothing");

    let items = session.describe_items(&[]).await.unwrap();

    assert!(items.is_empty(), "not the whole inventory");
    assert!(steam.inventory_asks().is_empty());
    assert!(steam.logons().is_empty(), "nor signs on for it");
}
