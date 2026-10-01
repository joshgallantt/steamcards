//! Playing the account's games against a stand-in for Steam: what Steam is
//! told, and what it says back.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::{AppId, GameRepository, Playing, PlayingRepository, PlayingSignal};
use game_data::{
    DefaultGameRepository, DefaultPlayingRepository, SteamGameClient, SteamPlayingClient,
};
use steam_api::{
    EResult, SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};

/// Playing as the app does it.
fn repository(session: Arc<SteamClient>) -> DefaultPlayingRepository {
    DefaultPlayingRepository::new(Arc::new(SteamPlayingClient::new(session)))
}

fn session(steam: &FakeSteam, name: &str) -> Arc<SteamClient> {
    let dir =
        std::env::temp_dir().join(format!("steamcards-playing-{name}-{}", std::process::id()));
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

/// Plays `app_id`, and waits for Steam to hear of it: told it once another
/// device plays, Steam would sign the session off.
async fn plays(repo: &DefaultPlayingRepository, steam: &FakeSteam, app_id: u32) {
    let told = steam.games_played().len();
    repo.play(&[AppId(app_id)], false).await.unwrap();
    eventually(|| steam.games_played().len() > told).await;
}

async fn signal(repo: &DefaultPlayingRepository) -> PlayingSignal {
    tokio::time::timeout(Duration::from_secs(1), repo.next_signal())
        .await
        .expect("a signal within a second")
}

#[tokio::test]
async fn games_are_played_and_told_again_only_when_they_change() {
    let steam = FakeSteam::start().await;
    let repo = repository(session(&steam, "play"));

    repo.play(&[AppId(620)], false).await.unwrap();
    repo.play(&[AppId(620)], false).await.unwrap();
    repo.play(&[AppId(620), AppId(440)], false).await.unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620], vec![620, 440]]);
    assert!(steam.statuses().is_empty(), "offline needs no saying");
}

#[tokio::test]
async fn appearing_online_is_said_and_unsaid() {
    let steam = FakeSteam::start().await;
    let repo = repository(session(&steam, "online"));

    repo.play(&[AppId(620)], true).await.unwrap();
    repo.play(&[AppId(620)], true).await.unwrap();
    repo.play(&[AppId(620)], false).await.unwrap();

    eventually(|| steam.statuses().len() == 2).await;
    assert_eq!(steam.statuses(), [1, 0]);
}

#[tokio::test]
async fn steams_word_on_playing_arrives_as_signals() {
    let steam = FakeSteam::start().await;
    let repo = repository(session(&steam, "signals"));
    plays(&repo, &steam, 620).await;

    steam.block(true, 730);
    assert_eq!(
        signal(&repo).await,
        PlayingSignal::Blocked(Some(AppId(730)))
    );
    assert_eq!(repo.playing(), Playing::Elsewhere(Some(AppId(730))));

    steam.block(false, 0);
    assert_eq!(signal(&repo).await, PlayingSignal::Unblocked);
    assert_eq!(repo.playing(), Playing::Here);

    steam.new_items(1);
    steam.hang_up();
    assert!(
        matches!(signal(&repo).await, PlayingSignal::Lost(_)),
        "new items are the card component's to hear"
    );
}

#[tokio::test]
async fn another_session_taking_over_is_its_own_signal() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "replaced");
    let repo = repository(session.clone());
    repo.play(&[AppId(620)], false).await.unwrap();

    steam.sign_off(EResult::LOGON_SESSION_REPLACED);

    assert_eq!(signal(&repo).await, PlayingSignal::Replaced);
    let library = DefaultGameRepository::new(Arc::new(SteamGameClient::new(session)));
    assert!(
        library.replaced(),
        "and the library says so after, for a read that failed meanwhile"
    );
}

#[tokio::test]
async fn nothing_is_played_while_another_device_plays() {
    let steam = FakeSteam::start().await;
    steam.busy_elsewhere(730);
    let repo = repository(session(&steam, "busy"));

    repo.play(&[AppId(620)], false).await.unwrap();
    assert_eq!(
        repo.playing(),
        Playing::Elsewhere(Some(AppId(730))),
        "Steam said so as it signed on"
    );

    steam.block(false, 0);
    assert_eq!(signal(&repo).await, PlayingSignal::Unblocked);
    repo.play(&[AppId(620)], false).await.unwrap();

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
    let repo = repository(session(&steam, "told-again"));
    plays(&repo, &steam, 620).await;
    steam.block(true, 730);
    assert_eq!(
        signal(&repo).await,
        PlayingSignal::Blocked(Some(AppId(730)))
    );
    steam.block(false, 0);
    assert_eq!(signal(&repo).await, PlayingSignal::Unblocked);

    repo.play(&[AppId(620)], false).await.unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620], vec![620]]);
}

#[tokio::test]
async fn another_device_taking_over_is_its_own_signal() {
    let steam = FakeSteam::start().await;
    let repo = repository(session(&steam, "taken-over"));
    repo.play(&[AppId(620)], false).await.unwrap();
    eventually(|| steam.games_played().len() == 1).await;

    steam.take_over(730);

    assert_eq!(signal(&repo).await, PlayingSignal::TakenOver);
    assert_eq!(repo.playing(), Playing::Here, "signed off, nothing is said");
}

#[tokio::test]
async fn listening_signs_on_to_hear_another_device_without_playing() {
    let steam = FakeSteam::start().await;
    steam.busy_elsewhere(730);
    let repo = repository(session(&steam, "listen"));

    repo.listen().await.unwrap();
    repo.listen().await.unwrap();
    assert_eq!(repo.playing(), Playing::Elsewhere(Some(AppId(730))));
    steam.block(false, 0);

    assert_eq!(signal(&repo).await, PlayingSignal::Unblocked);
    assert_eq!(steam.logons().len(), 1, "signed on once");
    assert!(steam.games_played().is_empty());
}

#[tokio::test]
async fn after_a_drop_the_new_connection_is_told_everything() {
    let steam = FakeSteam::start().await;
    let repo = repository(session(&steam, "again"));
    repo.play(&[AppId(620)], true).await.unwrap();
    // Frames still on their way when a connection drops are lost, as they
    // would be: let these arrive first.
    eventually(|| steam.games_played().len() == 1).await;
    steam.hang_up();
    assert!(matches!(signal(&repo).await, PlayingSignal::Lost(_)));

    repo.play(&[AppId(620)], true).await.unwrap();

    eventually(|| steam.games_played().len() == 2).await;
    assert_eq!(steam.games_played(), [vec![620], vec![620]]);
    assert_eq!(steam.statuses(), [1, 1]);
    assert_eq!(steam.logons().len(), 2);
}

#[tokio::test]
async fn stopping_stops_the_games_and_signs_off() {
    let steam = FakeSteam::start().await;
    let repo = repository(session(&steam, "stop"));
    repo.play(&[AppId(620)], false).await.unwrap();

    repo.stop().await;

    assert_eq!(steam.games_played(), [vec![620], vec![]]);
    assert_eq!(steam.log_offs(), 1);
}
