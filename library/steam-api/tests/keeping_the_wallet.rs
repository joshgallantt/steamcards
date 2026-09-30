//! The wallet, which Steam tells over the CM connection as a session signs
//! on, against a stand-in for Steam.

use std::{sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    SteamClient,
    cm::WalletInfo,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};
use wiremock::MockServer;

/// A session signed in as the stand-in's account, with the site at `site`.
fn signed_in(steam: &FakeSteam, site: &MockServer, name: &str) -> SteamClient {
    let dir = std::env::temp_dir().join(format!("steamcards-wallet-{name}-{}", std::process::id()));
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
    let mut endpoints = steam.endpoints();
    endpoints.community = site.uri();
    SteamClient::with_endpoints(store, &DebugLog::off(), endpoints)
}

/// Waits until the signed-on connection has heard of the wallet.
async fn wallet_told(session: &SteamClient) {
    for _ in 0..100 {
        if session.current().and_then(|c| c.wallet()).is_some() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Steam never told of the wallet");
}

#[tokio::test]
async fn the_wallet_is_kept_when_the_connection_goes_unasked() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(2);
    let site = MockServer::start().await;
    let session = signed_in(&steam, &site, "wallet-kept");
    let pounds = WalletInfo {
        has_wallet: true,
        currency: 2,
    };

    session.connection().await.unwrap();
    wallet_told(&session).await;
    session.disconnect().await;
    assert_eq!(session.wallet(), Some(pounds), "signed off: a pause");

    session.connection().await.unwrap();
    wallet_told(&session).await;
    steam.hang_up();
    steam.wallet_unsaid();
    while session.current().is_some() {
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    session.connection().await.unwrap();
    assert_eq!(
        session.wallet(),
        Some(pounds),
        "a lost connection's, until a new one says"
    );
}

#[tokio::test]
async fn the_wallet_is_what_steam_says_as_a_session_signs_on() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(2);
    let site = MockServer::start().await;
    let session = signed_in(&steam, &site, "wallet");
    assert_eq!(session.wallet(), None, "not signed on yet");

    session.connection().await.unwrap();

    let pounds = WalletInfo {
        has_wallet: true,
        currency: 2,
    };
    assert_eq!(session.wallet(), Some(pounds));
    session.disconnect().await;
    assert_eq!(
        session.wallet(),
        Some(pounds),
        "kept when the connection goes"
    );
    session.forget().unwrap();
    assert_eq!(session.wallet(), None, "forgotten with the sign-in");
}
