//! Signing on and playing, against a stand-in for Steam: what the session
//! tells Steam, and what it hears back.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    EResult, Session,
    cm::{Blocked, Event},
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use tokio::sync::broadcast;

/// A session signed in as the stand-in's account, with a sign-in good for
/// months.
fn signed_in(steam: &FakeSteam, name: &str) -> (Session, Arc<ConfigFile>) {
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
    let session = Session::with_endpoints(store.clone(), &DebugLog::off(), steam.endpoints());
    (session, store)
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
async fn games_are_played_until_told_otherwise() {
    let steam = FakeSteam::start().await;
    let (session, _) = signed_in(&steam, "play");

    let conn = session.connection().await.unwrap();
    conn.play(&[620, 440]).unwrap();
    conn.play(&[]).unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620, 440], vec![]]);
}

#[tokio::test]
async fn at_most_32_games_play_at_once() {
    let steam = FakeSteam::start().await;
    let (session, _) = signed_in(&steam, "most");

    let conn = session.connection().await.unwrap();
    let many: Vec<u32> = (1..=40).collect();
    conn.play(&many).unwrap();

    eventually(|| !steam.games_played().is_empty()).await;
    assert_eq!(steam.games_played()[0].len(), 32);
}

#[tokio::test]
async fn a_session_is_offline_until_it_says_otherwise() {
    let steam = FakeSteam::start().await;
    let (session, _) = signed_in(&steam, "online");

    let conn = session.connection().await.unwrap();
    conn.play(&[620]).unwrap();
    conn.set_online(true).unwrap();
    conn.set_online(false).unwrap();

    eventually(|| steam.statuses().len() == 2).await;
    assert_eq!(steam.statuses(), [1, 0], "nothing is said until asked");
}

#[tokio::test]
async fn another_device_playing_is_heard_at_sign_on_and_after() {
    let steam = FakeSteam::start().await;
    steam.busy_elsewhere(730);
    let (session, _) = signed_in(&steam, "blocked");

    let conn = session.connection().await.unwrap();
    let mut events = conn.events();
    eventually(|| conn.blocked().is_some()).await;
    assert_eq!(
        conn.blocked(),
        Some(Blocked {
            blocked: true,
            app_id: Some(730),
        }),
        "said at sign-on, inside a Multi"
    );

    steam.block(false, 0);
    assert_eq!(
        next(&mut events).await,
        Event::PlayingBlocked(Blocked {
            blocked: false,
            app_id: None,
        })
    );
}

#[tokio::test]
async fn new_items_and_sign_offs_are_heard() {
    let steam = FakeSteam::start().await;
    let (session, _) = signed_in(&steam, "events");
    let conn = session.connection().await.unwrap();
    let mut events = conn.events();

    steam.new_items(2);
    assert_eq!(next(&mut events).await, Event::NewItems(2));

    steam.sign_off(EResult::LOGGED_IN_ELSEWHERE);
    assert_eq!(
        next(&mut events).await,
        Event::LoggedOff(EResult::LOGGED_IN_ELSEWHERE)
    );
    assert!(!conn.is_signed_on());
}

#[tokio::test]
async fn a_dropped_connection_signs_on_again_when_next_needed() {
    let steam = FakeSteam::start().await;
    let (session, _) = signed_in(&steam, "reconnect");
    let first = session.connection().await.unwrap();
    let mut events = first.events();

    steam.hang_up();
    assert_eq!(next(&mut events).await, Event::Closed);

    let second = session.connection().await.unwrap();
    assert!(second.is_signed_on());
    assert_eq!(steam.logons().len(), 2);
    second.play(&[620]).unwrap();
    eventually(|| steam.games_played().len() == 1).await;
}

#[tokio::test]
async fn a_rejected_sign_in_is_marked_and_not_tried_again() {
    let steam = FakeSteam::start().await;
    steam.refuse_logon(EResult::ACCESS_DENIED);
    let (session, _) = signed_in(&steam, "rejected");

    let e = session.connection().await.unwrap_err();
    assert!(e.to_string().contains("sign in again"), "{e}");
    assert!(session.is_rejected());

    session.connection().await.unwrap_err();
    assert_eq!(steam.logons().len(), 1, "not tried again");
}

#[tokio::test]
async fn a_busy_steam_is_not_a_rejection() {
    let steam = FakeSteam::start().await;
    steam.refuse_logon(EResult::SERVICE_UNAVAILABLE);
    let (session, _) = signed_in(&steam, "busy");

    session.connection().await.unwrap_err();
    assert!(!session.is_rejected());
}

#[tokio::test]
async fn signing_off_tells_steam() {
    let steam = FakeSteam::start().await;
    let (session, _) = signed_in(&steam, "logoff");
    session.connection().await.unwrap();

    session.disconnect().await;

    assert_eq!(steam.log_offs(), 1);
    assert!(session.current().is_none());
}
