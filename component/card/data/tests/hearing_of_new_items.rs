//! New items as Steam announces them, on whichever connection is signed on,
//! against a stand-in for Steam: each card passed on once, and what was new
//! before steamcards first signed on left alone.

use std::{sync::Arc, time::Duration};

use card::{AssetId, CardRepository, NewItem};
use card_data::{DefaultCardRepository, SteamCardClient};
use chrono::DateTime;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::AppId;
use steam_api::{
    SteamClient,
    cm::UnseenItem,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token, unseen_card},
};

/// The cards as the app reads them.
fn repository(session: Arc<SteamClient>) -> DefaultCardRepository {
    DefaultCardRepository::new(Arc::new(SteamCardClient::new(session)))
}

fn session(steam: &FakeSteam, name: &str) -> Arc<SteamClient> {
    let dir = std::env::temp_dir().join(format!(
        "steamcards-new-items-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let file = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    file.save_credentials(Credentials {
        refresh_token: token(STEAM_ID, 4_000_000_000),
        account_name: ACCOUNT.into(),
        steam_id: STEAM_ID,
        login_id: 7,
    })
    .unwrap();
    Arc::new(SteamClient::with_endpoints(
        file,
        &DebugLog::off(),
        steam.endpoints(),
    ))
}

/// Waits until `check` holds, or a second has passed.
async fn eventually(check: impl Fn() -> bool) {
    for _ in 0..100 {
        if check() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(check(), "never happened");
}

/// Signs on, for whatever reason, and waits for Steam to say what was new
/// already.
async fn signs_on(session: &SteamClient) {
    let conn = session.connection().await.unwrap();
    eventually(|| conn.new_at_sign_on().is_some()).await;
}

async fn new_items(repo: &DefaultCardRepository) -> Vec<NewItem> {
    tokio::time::timeout(Duration::from_secs(1), repo.next_new_items())
        .await
        .expect("new items within a second")
}

/// A card Steam listed, as it's heard of.
fn heard(card: UnseenItem) -> NewItem {
    NewItem {
        asset_id: AssetId(card.asset_id),
        app_id: card.source_app_id.map(AppId),
        gained_at: card
            .gained_at
            .and_then(|at| DateTime::from_timestamp(at.into(), 0)),
    }
}

#[tokio::test]
async fn each_new_item_is_passed_on_once() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "once");
    let repo = repository(session.clone());
    signs_on(&session).await;

    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);
    assert_eq!(new_items(&repo).await, [heard(madison)]);

    let scott = unseen_card(31_003, 960_910);
    steam.announce(vec![scott]);
    assert_eq!(
        new_items(&repo).await,
        [heard(scott)],
        "Steam lists Madison again, until the inventory is viewed"
    );
}

#[tokio::test]
async fn items_new_before_signing_on_are_not_passed_on() {
    let steam = FakeSteam::start().await;
    let earlier = unseen_card(31_001, 960_910);
    steam.already_unseen(vec![earlier]);
    let session = session(&steam, "before");
    let repo = repository(session.clone());
    signs_on(&session).await;

    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);

    assert_eq!(new_items(&repo).await, [heard(madison)]);
}

#[tokio::test]
async fn a_count_alone_says_to_look_when_it_goes_up() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "count");
    let repo = repository(session.clone());
    signs_on(&session).await;

    steam.new_items(1);
    assert_eq!(new_items(&repo).await, []);

    steam.new_items(1);
    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);
    assert_eq!(
        new_items(&repo).await,
        [heard(madison)],
        "the same count again is nothing new"
    );

    steam.new_items(2);
    assert_eq!(new_items(&repo).await, []);
}

#[tokio::test]
async fn items_in_other_inventories_are_not_passed_on() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "other");
    let repo = repository(session.clone());
    signs_on(&session).await;

    let hat = UnseenItem {
        app_id: 440,
        context_id: 2,
        ..unseen_card(4_001, 440)
    };
    steam.announce(vec![hat]);

    assert_eq!(
        new_items(&repo).await,
        [],
        "no card, though one more new item: a look is cheap"
    );
}

#[tokio::test]
async fn a_new_connection_passes_on_only_what_is_new_since() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "again");
    let repo = repository(session.clone());
    signs_on(&session).await;
    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);
    assert_eq!(new_items(&repo).await, [heard(madison)]);

    steam.hang_up();
    eventually(|| session.current().is_none()).await;
    signs_on(&session).await;
    let scott = unseen_card(31_003, 960_910);
    steam.announce(vec![scott]);

    assert_eq!(new_items(&repo).await, [heard(scott)]);
}

#[tokio::test]
async fn an_item_that_arrived_while_signed_off_is_passed_on_when_signed_on_again() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "meanwhile");
    let repo = repository(session.clone());
    signs_on(&session).await;

    // Paused: farming stops, and signs off. A card drops meanwhile, on
    // another device.
    session.disconnect().await;
    let madison = unseen_card(31_002, 960_910);
    steam.already_unseen(vec![madison]);
    signs_on(&session).await;

    assert_eq!(
        new_items(&repo).await,
        [heard(madison)],
        "what was new at sign-on, since last time"
    );
}
