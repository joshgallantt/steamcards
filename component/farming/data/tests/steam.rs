//! The farming repositories against stand-ins for Steam and its community
//! site.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use farming::{PlayRepository, Signal};
use farming_data::SteamPlayRepository;
use steam_api::{
    EResult, Session,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
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

#[tokio::test]
async fn games_are_played_and_told_again_only_when_they_change() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "play"));

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
    let repo = SteamPlayRepository::new(session(&steam, "online"));

    repo.play(&[620], true).await.unwrap();
    repo.play(&[620], true).await.unwrap();
    repo.play(&[620], false).await.unwrap();

    eventually(|| steam.statuses().len() == 2).await;
    assert_eq!(steam.statuses(), [1, 0]);
}

#[tokio::test]
async fn steams_news_arrives_as_signals() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "signals"));
    repo.play(&[620], false).await.unwrap();

    steam.block(true, 730);
    assert_eq!(signal(&repo).await, Signal::Blocked(Some(730)));
    assert_eq!(repo.blocked(), Some(Some(730)));

    steam.block(false, 0);
    assert_eq!(signal(&repo).await, Signal::Unblocked);
    assert_eq!(repo.blocked(), None);

    steam.new_items(1);
    assert_eq!(signal(&repo).await, Signal::NewItems);

    steam.hang_up();
    assert!(matches!(signal(&repo).await, Signal::Lost(_)));
}

#[tokio::test]
async fn another_session_taking_over_is_its_own_signal() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "replaced"));
    repo.play(&[620], false).await.unwrap();

    steam.sign_off(EResult::LOGON_SESSION_REPLACED);

    assert_eq!(signal(&repo).await, Signal::Replaced);
}

#[tokio::test]
async fn after_a_drop_the_new_connection_is_told_everything() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "again"));
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
async fn stopping_stops_the_games_and_signs_off() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, "stop"));
    repo.play(&[620], false).await.unwrap();

    repo.stop().await;

    assert_eq!(steam.games_played(), [vec![620], vec![]]);
    assert_eq!(steam.log_offs(), 1);
}
