//! The farming repositories against stand-ins for Steam and its community
//! site.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use farming::{CardsRepository, Game, PlayRepository, Signal};
use farming_data::{SteamCardsRepository, SteamPlayRepository};
use steam_api::{
    Session,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

fn session(steam: &FakeSteam, site: Option<&MockServer>, name: &str) -> Arc<Session> {
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
    let mut endpoints = steam.endpoints();
    if let Some(site) = site {
        endpoints.community = site.uri();
    }
    Arc::new(Session::with_endpoints(file, &DebugLog::off(), endpoints))
}

fn fixture(name: &str) -> String {
    let at = format!(
        "{}/../../../library/steam-api/tests/pages/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(at).unwrap()
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
async fn badges_become_games() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let badges = format!("/profiles/{STEAM_ID}/badges");
    for (page, file) in [("1", "badges-1.html"), ("2", "badges-2.html")] {
        Mock::given(method("GET"))
            .and(path(badges.clone()))
            .and(query_param("p", page))
            .respond_with(ResponseTemplate::new(200).set_body_string(fixture(file)))
            .mount(&site)
            .await;
    }
    Mock::given(method("GET"))
        .and(path(format!("/profiles/{STEAM_ID}/gamecards/730")))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture("gamecards-730.html")))
        .mount(&site)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/profiles/{STEAM_ID}/gamecards/440")))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html></html>"))
        .mount(&site)
        .await;
    let repo = SteamCardsRepository::new(session(&steam, Some(&site), "cards"));

    let games = repo.games().await.unwrap();

    let portal = games.iter().find(|g| g.app_id == 620).unwrap();
    assert_eq!(
        portal,
        &Game {
            app_id: 620,
            name: "Portal 2".into(),
            hours: 5.2,
            cards_left: 3,
            cards_dropped: 1,
        }
    );
    assert_eq!(games.iter().filter(|g| !g.is_done()).count(), 4);
    assert_eq!(repo.game(730).await.unwrap().cards_left, 2);
}

#[tokio::test]
async fn games_are_played_and_told_again_only_when_they_change() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, None, "play"));

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
    let repo = SteamPlayRepository::new(session(&steam, None, "online"));

    repo.play(&[620], true).await.unwrap();
    repo.play(&[620], true).await.unwrap();
    repo.play(&[620], false).await.unwrap();

    eventually(|| steam.statuses().len() == 2).await;
    assert_eq!(steam.statuses(), [1, 0]);
}

#[tokio::test]
async fn steams_news_arrives_as_signals() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, None, "signals"));
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
async fn after_a_drop_the_new_connection_is_told_everything() {
    let steam = FakeSteam::start().await;
    let repo = SteamPlayRepository::new(session(&steam, None, "again"));
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
    let repo = SteamPlayRepository::new(session(&steam, None, "stop"));
    repo.play(&[620], false).await.unwrap();

    repo.stop().await;

    assert_eq!(steam.games_played(), [vec![620], vec![]]);
    assert_eq!(steam.log_offs(), 1);
}
