//! Someone looking at their Steam library in steamcards: the game component
//! wired as the composition root wires it, over the real data layer. Only
//! Steam and steamcommunity.com are stood in for, by stand-ins that serve
//! the badge pages as the site does.

use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::{GameError, SteamLibrary};
use game_di::GameComponent;
use steam_api::{
    SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

pub(crate) struct Player {
    pub(crate) site: MockServer,
    _steam: FakeSteam,
    game: GameComponent,
}

impl Player {
    /// Someone signed in, with the site saying nothing yet.
    pub(crate) async fn new(name: &str) -> Self {
        let steam = FakeSteam::start().await;
        let site = MockServer::start().await;
        let dir =
            std::env::temp_dir().join(format!("steamcards-game-{name}-{}", std::process::id()));
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
        let client = SteamClient::with_endpoints(file, &DebugLog::off(), endpoints);
        Self {
            game: GameComponent::new(Arc::new(client)),
            site,
            _steam: steam,
        }
    }

    /// The site shows the account's two pages of badges, and the card pages
    /// of the games they may be wrong about.
    pub(crate) async fn has_badges(&self) {
        let badges = format!("/profiles/{STEAM_ID}/badges");
        self.serves(&badges, ("p", "1"), page("badges-1.html"))
            .await;
        self.serves(&badges, ("p", "2"), page("badges-2.html"))
            .await;
        let cards = |app_id| format!("/profiles/{STEAM_ID}/gamecards/{app_id}");
        self.serves(
            &cards(730),
            ("l", "english"),
            card_page("gamecards-730.html"),
        )
        .await;
        self.serves(&cards(440), ("l", "english"), "<html></html>".into())
            .await;
    }

    pub(crate) async fn reads_the_library(&self) -> Result<SteamLibrary, GameError> {
        self.game.read_library.call().await.unwrap()
    }

    async fn serves(&self, at: &str, query: (&str, &str), page: String) {
        Mock::given(method("GET"))
            .and(path(at.to_owned()))
            .and(query_param(query.0, query.1))
            .respond_with(ResponseTemplate::new(200).set_body_string(page))
            .mount(&self.site)
            .await;
    }
}

/// A badge page, as the game data crate keeps it.
fn page(name: &str) -> String {
    let at = format!("{}/../data/tests/pages/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(at).unwrap()
}

/// A game's card page, as steam-api keeps it.
fn card_page(name: &str) -> String {
    let at = format!(
        "{}/../../../library/steam-api/tests/pages/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(at).unwrap()
}
