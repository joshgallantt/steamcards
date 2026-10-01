//! The wallet behind the account domain, as Steam tells of it when a
//! session signs on, against a stand-in for Steam.

use std::sync::Arc;

use account::{AccountRepository, Wallet};
use account_data::{DefaultAccountRepository, SteamAccountClient};
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use money::Currency;
use steam_api::{
    SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};

/// The account's repository over a session signed in as the stand-in's
/// account, and the session.
fn signed_in(steam: &FakeSteam, name: &str) -> (DefaultAccountRepository, Arc<SteamClient>) {
    let dir = std::env::temp_dir().join(format!("steamcards-wallet-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    file.save_credentials(Credentials {
        refresh_token: token(STEAM_ID, 4_000_000_000),
        account_name: ACCOUNT.into(),
        steam_id: STEAM_ID,
        login_id: 7,
    })
    .unwrap();
    let session = Arc::new(SteamClient::with_endpoints(
        file,
        &DebugLog::off(),
        steam.endpoints(),
    ));
    let client = SteamAccountClient::new(session.clone());
    (DefaultAccountRepository::new(Arc::new(client)), session)
}

#[tokio::test]
async fn the_wallet_is_the_one_steam_tells_of_with_valves_fees() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(3);
    let (repo, session) = signed_in(&steam, "euros");
    assert_eq!(repo.wallet(), None, "not signed on yet");

    session.connection().await.unwrap();

    let euros = repo.wallet().unwrap();
    assert_eq!(euros, Wallet::new(Currency::EUR));
    assert_eq!((euros.steam_fee, euros.publisher_fee), (500, 1_000));
}

#[tokio::test]
async fn an_account_without_a_wallet_is_priced_in_dollars() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(0);
    let (repo, session) = signed_in(&steam, "none");

    session.connection().await.unwrap();

    assert_eq!(repo.wallet(), Some(Wallet::new(Currency::USD)));
}
