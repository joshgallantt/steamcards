//! New items in the account's inventory, against a stand-in for Steam: what
//! a session asks at sign-on, and what Steam says as items arrive.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    Session,
    cm::{Announcement, Event},
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token, unseen_card},
};
use tokio::sync::broadcast;

/// A session signed in as the stand-in's account, with a sign-in good for
/// months.
fn signed_in(steam: &FakeSteam, name: &str) -> Session {
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
    Session::with_endpoints(store, &DebugLog::off(), steam.endpoints())
}

/// The next event, within a second.
async fn next(events: &mut broadcast::Receiver<Event>) -> Event {
    tokio::time::timeout(Duration::from_secs(1), events.recv())
        .await
        .expect("an event within a second")
        .expect("the connection's events")
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

#[tokio::test]
async fn signing_on_asks_what_is_new_already() {
    let steam = FakeSteam::start().await;
    let before = unseen_card(31_001, 960_910);
    steam.already_unseen(vec![before]);
    let session = signed_in(&steam, "new-at-sign-on");

    let conn = session.connection().await.unwrap();

    eventually(|| conn.new_at_sign_on().is_some()).await;
    assert_eq!(steam.announcement_asks(), 1, "asked once, at sign-on");
    assert_eq!(
        conn.new_at_sign_on(),
        Some(Announcement {
            count: 1,
            items: vec![before],
            at_sign_on: true,
        }),
        "what was new before this session"
    );
}

#[tokio::test]
async fn new_items_come_with_their_ids_until_the_inventory_is_viewed() {
    let steam = FakeSteam::start().await;
    let session = signed_in(&steam, "new-items");
    let conn = session.connection().await.unwrap();
    eventually(|| conn.new_at_sign_on().is_some()).await;
    let mut events = conn.events();

    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);
    assert_eq!(
        next(&mut events).await,
        Event::NewItems(Announcement {
            count: 1,
            items: vec![madison],
            at_sign_on: false,
        })
    );

    let scott = unseen_card(31_003, 960_910);
    steam.announce(vec![scott]);
    assert_eq!(
        next(&mut events).await,
        Event::NewItems(Announcement {
            count: 2,
            items: vec![madison, scott],
            at_sign_on: false,
        }),
        "Madison again: nobody has looked at the inventory"
    );

    steam.inventory_viewed();
    assert_eq!(
        next(&mut events).await,
        Event::NewItems(Announcement {
            count: 0,
            items: Vec::new(),
            at_sign_on: false,
        })
    );
}
