//! Someone looking at their cards in steamcards: the card component wired as
//! the composition root wires it, over the real data layer. Only Steam and
//! steamcommunity.com are stood in for: the site serves card pages, and
//! Steam holds the account's items.

use std::sync::Arc;

use card::{AssetId, CardAsset, CardError, CardSet, GameCards};
use card_di::CardComponent;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::AppId;
use steam_api::{
    SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

pub(crate) struct Player {
    pub(crate) steam: FakeSteam,
    pub(crate) site: MockServer,
    card: CardComponent,
}

impl Player {
    /// Someone signed in, holding nothing yet.
    pub(crate) async fn new(name: &str) -> Self {
        let steam = FakeSteam::start().await;
        let site = MockServer::start().await;
        let dir =
            std::env::temp_dir().join(format!("steamcards-card-{name}-{}", std::process::id()));
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
            card: CardComponent::new(Arc::new(client)),
            steam,
            site,
        }
    }

    /// The site shows Counter-Strike 2's card page, or its foil badge's.
    pub(crate) async fn has_counter_strikes_card_page(&self, foil: bool) {
        let (border, page) = if foil {
            ("1", "gamecards-730-foil.html")
        } else {
            ("0", "gamecards-730.html")
        };
        let mock =
            Mock::given(method("GET")).and(path(format!("/profiles/{STEAM_ID}/gamecards/730")));
        let mock = if foil {
            mock.and(query_param("border", border))
        } else {
            mock
        };
        mock.respond_with(ResponseTemplate::new(200).set_body_string(card_page(page)))
            .mount(&self.site)
            .await;
    }

    pub(crate) async fn looks_at_cards(&self, app_id: u32) -> Result<GameCards, CardError> {
        self.card.look_at_cards.call(AppId(app_id)).await.unwrap()
    }

    pub(crate) async fn looks_at_foils(&self, app_id: u32) -> Result<CardSet, CardError> {
        self.card.look_at_foils.call(AppId(app_id)).await.unwrap()
    }

    /// Which cards these new items are.
    pub(crate) async fn identifies(&self, asset_ids: &[u64]) -> Result<Vec<CardAsset>, CardError> {
        let ids = asset_ids.iter().copied().map(AssetId).collect();
        self.card.identify_cards.call(ids).await.unwrap()
    }
}

/// A game's card page, as steam-api keeps it.
fn card_page(name: &str) -> String {
    let at = format!(
        "{}/../../../library/steam-api/tests/pages/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(at).unwrap()
}
