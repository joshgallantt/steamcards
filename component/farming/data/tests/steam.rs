//! The farming repositories against stand-ins for Steam and its community
//! site.

use std::{sync::Arc, time::Duration};

use chrono::DateTime;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use farming::{NewItem, PlayRepository, Signal};
use farming_data::SteamPlayRepository;
use keep_awake::KeepAwake;
use steam_api::{
    EResult, Session,
    cm::UnseenItem,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token, unseen_card},
};

fn session(steam: &FakeSteam, name: &str) -> Arc<Session> {
    let dir =
        std::env::temp_dir().join(format!("steamcards-farming-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    file.save_credentials(Credentials {
        refresh_token: token(STEAM_ID, 4_000_000_000),
        account_name: ACCOUNT.into(),
        steam_id: STEAM_ID,
        login_id: 7,
    })
    .unwrap();
    Arc::new(Session::with_endpoints(
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

async fn signal(repo: &SteamPlayRepository) -> Signal {
    tokio::time::timeout(Duration::from_secs(1), repo.next_signal())
        .await
        .expect("a signal within a second")
}

/// Plays `app_id`, and waits for Steam to say what was new already.
async fn plays(repo: &SteamPlayRepository, session: &Session, app_id: u32) {
    repo.play(&[app_id], false).await.unwrap();
    eventually(|| {
        session
            .current()
            .is_some_and(|c| c.new_at_sign_on().is_some())
    })
    .await;
}

/// A card Steam listed, as farming hears of it.
fn heard(card: UnseenItem) -> NewItem {
    NewItem {
        asset_id: card.asset_id,
        app_id: card.source_app_id,
        gained_at: card
            .gained_at
            .and_then(|at| DateTime::from_timestamp(at.into(), 0)),
    }
}

#[tokio::test]
async fn games_are_played_and_told_again_only_when_they_change() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "play"), Arc::new(KeepAwake::off()));

    repo.play(&[620], false).await.unwrap();
    repo.play(&[620], false).await.unwrap();
    repo.play(&[620, 440], false).await.unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620], vec![620, 440]]);
    assert!(steam.statuses().is_empty(), "offline needs no saying");
}

#[tokio::test]
async fn appearing_online_is_said_and_unsaid() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "online"), Arc::new(KeepAwake::off()));

    repo.play(&[620], true).await.unwrap();
    repo.play(&[620], true).await.unwrap();
    repo.play(&[620], false).await.unwrap();

    eventually(|| steam.statuses().len() == 2).await;
    assert_eq!(steam.statuses(), [1, 0]);
}

#[tokio::test]
async fn steams_news_arrives_as_signals() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "signals");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 620).await;

    steam.block(true, 730);
    assert_eq!(signal(&repo).await, Signal::Blocked(Some(730)));
    assert_eq!(repo.blocked(), Some(Some(730)));

    steam.block(false, 0);
    assert_eq!(signal(&repo).await, Signal::Unblocked);
    assert_eq!(repo.blocked(), None);

    steam.new_items(1);
    assert_eq!(signal(&repo).await, Signal::NewItems(Vec::new()));

    steam.hang_up();
    assert!(matches!(signal(&repo).await, Signal::Lost(_)));
}

#[tokio::test]
async fn each_new_item_is_passed_on_once() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "items-once");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 960_910).await;

    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);
    assert_eq!(signal(&repo).await, Signal::NewItems(vec![heard(madison)]));

    let scott = unseen_card(31_003, 960_910);
    steam.announce(vec![scott]);
    assert_eq!(
        signal(&repo).await,
        Signal::NewItems(vec![heard(scott)]),
        "Steam lists Madison again, until the inventory is viewed"
    );
}

#[tokio::test]
async fn items_new_before_signing_on_are_not_passed_on() {
    let steam = FakeSteam::start().await;
    let earlier = unseen_card(31_001, 960_910);
    steam.already_unseen(vec![earlier]);
    let session = session(&steam, "items-before");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 960_910).await;

    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);

    assert_eq!(signal(&repo).await, Signal::NewItems(vec![heard(madison)]));
}

#[tokio::test]
async fn a_count_alone_says_to_look_when_it_goes_up() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "count");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 620).await;

    steam.new_items(1);
    assert_eq!(signal(&repo).await, Signal::NewItems(Vec::new()));

    steam.new_items(1);
    steam.block(true, 730);
    assert_eq!(
        signal(&repo).await,
        Signal::Blocked(Some(730)),
        "the same count again is nothing new"
    );

    steam.new_items(2);
    assert_eq!(signal(&repo).await, Signal::NewItems(Vec::new()));
}

#[tokio::test]
async fn items_in_other_inventories_are_not_passed_on() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "other-items");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 620).await;

    let hat = UnseenItem {
        app_id: 440,
        context_id: 2,
        ..unseen_card(4_001, 440)
    };
    steam.announce(vec![hat]);

    assert_eq!(
        signal(&repo).await,
        Signal::NewItems(Vec::new()),
        "no card, though one more new item: a look is cheap"
    );
}

#[tokio::test]
async fn a_new_connection_passes_on_only_what_is_new_since() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "items-again");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 960_910).await;
    let madison = unseen_card(31_002, 960_910);
    steam.announce(vec![madison]);
    assert_eq!(signal(&repo).await, Signal::NewItems(vec![heard(madison)]));

    steam.hang_up();
    assert!(matches!(signal(&repo).await, Signal::Lost(_)));
    plays(&repo, &session, 960_910).await;
    let scott = unseen_card(31_003, 960_910);
    steam.announce(vec![scott]);

    assert_eq!(signal(&repo).await, Signal::NewItems(vec![heard(scott)]));
}

#[tokio::test]
async fn an_item_that_arrived_while_signed_off_is_passed_on_when_signed_on_again() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "items-meanwhile");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 960_910).await;

    // Paused: farming stops, and signs off. A card drops meanwhile, on
    // another device.
    repo.stop().await;
    let madison = unseen_card(31_002, 960_910);
    steam.already_unseen(vec![madison]);
    plays(&repo, &session, 960_910).await;
    steam.block(true, 730);

    assert_eq!(
        signal(&repo).await,
        Signal::NewItems(vec![heard(madison)]),
        "what was new at sign-on, since last time"
    );
    assert_eq!(signal(&repo).await, Signal::Blocked(Some(730)));
}

#[tokio::test]
async fn another_session_taking_over_is_its_own_signal() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "replaced"), Arc::new(KeepAwake::off()));
    repo.play(&[620], false).await.unwrap();

    steam.sign_off(EResult::LOGON_SESSION_REPLACED);

    assert_eq!(signal(&repo).await, Signal::Replaced);
}

#[tokio::test]
async fn nothing_is_played_while_another_device_plays() {
    let steam = FakeSteam::start().await;
    steam.busy_elsewhere(730);
    let repo = SteamPlayRepository::new(session(&steam, "busy"), Arc::new(KeepAwake::off()));

    repo.play(&[620], false).await.unwrap();
    assert_eq!(
        repo.blocked(),
        Some(Some(730)),
        "Steam said so as it signed on"
    );

    steam.block(false, 0);
    assert_eq!(signal(&repo).await, Signal::Unblocked);
    repo.play(&[620], false).await.unwrap();

    eventually(|| !steam.games_played().is_empty()).await;
    assert_eq!(
        steam.games_played(),
        [vec![620]],
        "told once it stopped, and not before: Steam would have signed it off"
    );
    assert_eq!(steam.logons().len(), 1);
}

#[tokio::test]
async fn games_are_told_again_after_another_device_played() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "told-again");
    let repo = SteamPlayRepository::new(session.clone(), Arc::new(KeepAwake::off()));
    plays(&repo, &session, 620).await;
    steam.block(true, 730);
    assert_eq!(signal(&repo).await, Signal::Blocked(Some(730)));
    steam.block(false, 0);
    assert_eq!(signal(&repo).await, Signal::Unblocked);

    repo.play(&[620], false).await.unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620], vec![620]]);
}

#[tokio::test]
async fn another_device_taking_over_is_its_own_signal() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "taken-over"), Arc::new(KeepAwake::off()));
    repo.play(&[620], false).await.unwrap();
    eventually(|| steam.games_played().len() == 1).await;

    steam.take_over(730);

    assert_eq!(signal(&repo).await, Signal::TakenOver);
    assert_eq!(repo.blocked(), None, "signed off, nothing is said");
}

#[tokio::test]
async fn listening_signs_on_to_hear_another_device_without_playing() {
    let steam = FakeSteam::start().await;
    steam.busy_elsewhere(730);
    let repo = SteamPlayRepository::new(session(&steam, "listen"), Arc::new(KeepAwake::off()));

    repo.listen().await.unwrap();
    repo.listen().await.unwrap();
    assert_eq!(repo.blocked(), Some(Some(730)));
    steam.block(false, 0);

    assert_eq!(signal(&repo).await, Signal::Unblocked);
    assert_eq!(steam.logons().len(), 1, "signed on once");
    assert!(steam.games_played().is_empty());
}

#[tokio::test]
async fn after_a_drop_the_new_connection_is_told_everything() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "again"), Arc::new(KeepAwake::off()));
    repo.play(&[620], true).await.unwrap();
    // Frames still on their way when a connection drops are lost, as they
    // would be: let these arrive first.
    eventually(|| steam.games_played().len() == 1).await;
    steam.hang_up();
    assert!(matches!(signal(&repo).await, Signal::Lost(_)));

    repo.play(&[620], true).await.unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620], vec![620]]);
    assert_eq!(steam.statuses(), [1, 1]);
    assert_eq!(steam.logons().len(), 2);
}

#[tokio::test]
async fn the_computer_stays_awake_while_games_play() {
    let steam = FakeSteam::start().await;
    let awake = Arc::new(KeepAwake::running("sleep", &["60"]));
    let repo = SteamPlayRepository::new(session(&steam, "awake"), awake.clone());

    repo.play(&[620], false).await.unwrap();
    assert!(awake.is_held(), "held while playing");

    repo.play(&[], false).await.unwrap();
    assert!(!awake.is_held(), "let go with nothing to play");

    repo.play(&[620], false).await.unwrap();
    repo.stop().await;
    assert!(!awake.is_held(), "let go when farming stops");
}

#[tokio::test]
async fn stopping_stops_the_games_and_signs_off() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "stop"), Arc::new(KeepAwake::off()));
    repo.play(&[620], false).await.unwrap();

    repo.stop().await;

    assert_eq!(steam.games_played(), [vec![620], vec![]]);
    assert_eq!(steam.log_offs(), 1);
}
