//! Acceptance tier: the account's wallet as the user meets it, through the
//! account component as steamcards wires it, against a stand-in for Steam.

use std::sync::Arc;

use account_di::AccountComponent;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use money::Currency;
use steam_api::{
    SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, token},
};

#[tokio::test]
async fn the_wallet_is_what_steam_says_as_a_session_signs_on() {
    let steam = FakeSteam::start().await;
    steam.wallet_in(2);
    let dir = std::env::temp_dir().join(format!("steamcards-wallet-di-{}", std::process::id()));
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
    let account = AccountComponent::new(session.clone());
    assert_eq!(account.get_wallet.call(), None, "not signed on yet");

    // Steam signs the session on, as farming would.
    session.connection().await.unwrap();

    let pounds = account.get_wallet.call().unwrap();
    assert_eq!(pounds.currency, Currency::GBP);
    assert_eq!(pounds.seller_gets(62), 55, "Valve's fees");
}
