//! What the account holds, against a stand-in for Steam: its licences, the
//! apps in each package, and the games it marked private.

use std::{collections::HashMap, sync::Arc, time::Duration};

use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    EResult, SteamClient,
    test_support::{ACCOUNT, FakeSteam, STEAM_ID, bought, redeemed, token},
};

/// A session signed in as the stand-in's account, with a sign-in good for
/// months.
fn signed_in(steam: &FakeSteam, name: &str) -> SteamClient {
    let dir = std::env::temp_dir().join(format!("steamcards-{name}-{}", std::process::id()));
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
    SteamClient::with_endpoints(store, &DebugLog::off(), steam.endpoints())
        .with_sign_on_answer_within(Duration::from_millis(500))
}

/// When Portal 2 was bought: 2026-09-28.
const BOUGHT: u32 = 1_790_553_600;

#[tokio::test]
async fn the_licences_are_what_steam_lists_as_a_session_signs_on() {
    let steam = FakeSteam::start().await;
    let held = vec![bought(7, BOUGHT), redeemed(8, BOUGHT - 86_400)];
    steam.licensed(held.clone());
    let session = signed_in(&steam, "licences");

    assert_eq!(session.licences().await.unwrap(), held);
    assert!(held[0].is_purchase() && !held[1].is_purchase());
    assert_eq!(steam.logons().len(), 1, "signed on to hear them");
}

#[tokio::test]
async fn the_apps_in_each_package_come_from_steams_product_info() {
    let steam = FakeSteam::start().await;
    let held = [bought(7, BOUGHT), bought(8, BOUGHT), bought(9, BOUGHT)];
    steam.licensed(held.to_vec());
    steam.package(7, vec![620]);
    steam.package(8, vec![400, 323_180]);
    let session = signed_in(&steam, "packages");
    let asked: Vec<(u32, u64)> = held
        .iter()
        .map(|l| (l.package_id, l.access_token))
        .collect();

    let apps = session.package_apps(&asked).await.unwrap();

    assert_eq!(
        apps,
        HashMap::from([(7, vec![620]), (8, vec![400, 323_180])]),
        "every part of the answer, and nothing of a package Steam doesn't know"
    );
    assert_eq!(steam.product_info_asks(), [vec![7, 8, 9]], "in one ask");
}

#[tokio::test]
async fn a_package_asked_about_without_its_token_says_nothing() {
    let steam = FakeSteam::start().await;
    steam.licensed(vec![bought(7, BOUGHT)]);
    steam.package(7, vec![620]);
    let session = signed_in(&steam, "token");

    let apps = session.package_apps(&[(7, 0)]).await.unwrap();

    assert!(apps.is_empty(), "{apps:?}");
}

#[tokio::test]
async fn asking_about_no_packages_asks_steam_nothing() {
    let steam = FakeSteam::start().await;
    let session = signed_in(&steam, "no-packages");

    assert!(session.package_apps(&[]).await.unwrap().is_empty());
    assert!(steam.product_info_asks().is_empty());
    assert!(steam.logons().is_empty(), "not even signed on");
}

#[tokio::test]
async fn the_private_games_are_what_steam_says() {
    let steam = FakeSteam::start().await;
    steam.private(vec![620, 730]);
    let session = signed_in(&steam, "private");

    assert_eq!(session.private_apps().await.unwrap(), [620, 730]);
    assert_eq!(steam.private_asks(), 1);
}

#[tokio::test]
async fn nothing_signs_on_in_place_of_a_session_that_took_this_ones_place() {
    let steam = FakeSteam::start().await;
    let session = signed_in(&steam, "replaced");
    session.connection().await.unwrap();

    steam.sign_off(EResult::LOGON_SESSION_REPLACED);
    for _ in 0..200 {
        if session.replaced() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(session.replaced());

    let e = session.private_apps().await.unwrap_err();
    assert!(e.to_string().contains("in this one's place"), "{e}");
    session.licences().await.unwrap_err();
    session.package_apps(&[(7, 1_007)]).await.unwrap_err();
    assert_eq!(steam.logons().len(), 1, "not signed on again");
}
