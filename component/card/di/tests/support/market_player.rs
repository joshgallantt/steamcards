//! Someone farming with prices on screen in steamcards: the card component
//! wired as the composition root wires it, over the real data layer, a real
//! config file and a real prices file in a folder of their own. Only Steam
//! and its market are stood in for, and the market's pace is quick.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use card::{PriceBook, PriceEvent, system_clock};
use card_data::{FilePriceStore, MarketPace, SteamCardClient, SteamMarketClient};
use card_di::CardComponent;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use game::AppId;
use steam_api::{
    SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use wiremock::MockServer;

pub(crate) struct Player {
    pub(crate) steam: FakeSteam,
    pub(crate) site: MockServer,
    dir: PathBuf,
    card: CardComponent,
}

impl Player {
    /// Someone signed in, with a wallet in pounds, and nothing priced yet.
    pub(crate) async fn new(name: &str) -> Self {
        let steam = FakeSteam::start().await;
        steam.wallet_in(2);
        let site = MockServer::start().await;
        let dir =
            std::env::temp_dir().join(format!("steamcards-price-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        ConfigFile::open(dir.join("config.json"))
            .unwrap()
            .save_credentials(Credentials {
                refresh_token: token(STEAM_ID, 4_000_000_000),
                account_name: ACCOUNT.into(),
                steam_id: STEAM_ID,
                login_id: 7,
            })
            .unwrap();
        let card = start(&steam, &site, &dir);
        Self {
            steam,
            site,
            dir,
            card,
        }
    }

    /// Has these games priced, and waits for the first word of how it went.
    pub(crate) async fn has_prices_looked_up(&self, app_ids: &[u32]) -> PriceEvent {
        self.card
            .set_cards_to_price
            .call(app_ids.iter().copied().map(AppId).collect());
        let (tx, mut rx) = mpsc::channel(16);
        let token = CancellationToken::new();
        let pricing = self
            .card
            .keep_card_prices_up_to_date
            .call(token.clone(), tx);
        let event = tokio::time::timeout(Duration::from_secs(10), rx.recv())
            .await
            .expect("word within ten seconds")
            .expect("a word");
        token.cancel();
        pricing.await.unwrap();
        event
    }

    /// What's priced so far, as the screens show it.
    pub(crate) fn sees(&self) -> Arc<PriceBook> {
        self.card.get_card_prices.call()
    }

    /// Quits steamcards and starts it again.
    pub(crate) fn comes_back(self) -> Self {
        let card = start(&self.steam, &self.site, &self.dir);
        Self { card, ..self }
    }
}

/// steamcards starting: one config file, as the composition root opens it,
/// for the Steam client and the prices alike.
fn start(steam: &FakeSteam, site: &MockServer, dir: &Path) -> CardComponent {
    let file = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    let mut endpoints = steam.endpoints();
    endpoints.community = site.uri();
    let session = Arc::new(SteamClient::with_endpoints(
        file.clone(),
        &DebugLog::off(),
        endpoints,
    ));
    CardComponent::over(
        Arc::new(SteamCardClient::new(session.clone())),
        Arc::new(SteamMarketClient::with_pace(session, quick())),
        Arc::new(FilePriceStore::open(file, dir.join("prices.json"))),
        DebugLog::off(),
        system_clock(),
    )
}

/// The market's pace, a thousand times over.
fn quick() -> MarketPace {
    MarketPace {
        signed_in: Duration::from_millis(1),
        jitter: Duration::ZERO,
        signed_out: Duration::from_millis(1),
        first_pause: Duration::from_secs(20 * 60),
        longest_pause: Duration::from_secs(60 * 60),
        after_server_error: Duration::from_millis(10),
    }
}
