//! The library as steamcommunity.com's badge pages show it, against
//! stand-ins for Steam and the site.

use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::{CardDrops, Game, GameRepository};
use game_data::SteamGameRepository;
use steam_api::{
    Session,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

fn fixture(name: &str) -> String {
    let at = format!(
        "{}/../../../library/steam-api/tests/pages/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(at).unwrap()
}

async fn serving(site: &MockServer, at: String, page: String, query: Option<(&str, &str)>) {
    let mock = Mock::given(method("GET")).and(path(at));
    let mock = match query {
        Some((k, v)) => mock.and(query_param(k, v)),
        None => mock,
    };
    mock.respond_with(ResponseTemplate::new(200).set_body_string(page))
        .mount(site)
        .await;
}

async fn repository(steam: &FakeSteam, site: &MockServer, name: &str) -> SteamGameRepository {
    let dir = std::env::temp_dir().join(format!("steamcards-game-{name}-{}", std::process::id()));
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
    endpoints.community = site.uri();
    let session = Session::with_endpoints(file, &DebugLog::off(), endpoints);
    SteamGameRepository::new(Arc::new(session))
}

#[tokio::test]
async fn the_badges_are_the_library() {
    let steam = FakeSteam::start().await;
    let site = MockServer::start().await;
    let badges = format!("/profiles/{STEAM_ID}/badges");
    serving(
        &site,
        badges.clone(),
        fixture("badges-1.html"),
        Some(("p", "1")),
    )
    .await;
    serving(&site, badges, fixture("badges-2.html"), Some(("p", "2"))).await;
    serving(
        &site,
        format!("/profiles/{STEAM_ID}/gamecards/730"),
        fixture("gamecards-730.html"),
        None,
    )
    .await;
    serving(
        &site,
        format!("/profiles/{STEAM_ID}/gamecards/440"),
        "<html></html>".into(),
        None,
    )
    .await;
    let repo = repository(&steam, &site, "badges").await;

    let library = repo.library().await.unwrap();

    assert_eq!(
        library.game(620),
        Some(&Game {
            app_id: 620,
            name: "Portal 2".into(),
            hours: 5.2,
            drops: CardDrops {
                received: 1,
                remaining: 3,
            },
            badge_level: 1,
        })
    );
    assert_eq!(library.with_drops_left().count(), 5);
    assert_eq!(library.drops_left(), 3 + 6 + 4 + 2 + 1);
    let never_played = library.game(1086940).unwrap();
    assert_eq!(
        (never_played.drops.received, never_played.drops.total()),
        (0, 6),
        "nothing has dropped from a game never played"
    );
}
