//! The Steam sign-in behind the account domain, against a stand-in for Steam.

use std::{sync::Arc, time::Duration};

use account::{AccountRepository, LoginChallenge};
use account_data::SteamAccountRepository;
use config_file::{ConfigFile, CredentialStore, Credentials};
use debug_log::DebugLog;
use steam_api::{
    EResult, SteamClient,
    test_support::{ACCOUNT, FakeSteam, QrScript, STEAM_ID, token},
};
use tokio::sync::mpsc;

fn repository(steam: &FakeSteam, name: &str) -> (SteamAccountRepository, Arc<ConfigFile>) {
    let dir =
        std::env::temp_dir().join(format!("steamcards-account-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let file = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    let session = SteamClient::with_endpoints(file.clone(), &DebugLog::off(), steam.endpoints());
    (SteamAccountRepository::new(Arc::new(session)), file)
}

fn saved(file: &ConfigFile, expires_at: i64) {
    file.save_credentials(Credentials {
        refresh_token: token(STEAM_ID, expires_at),
        account_name: ACCOUNT.into(),
        steam_id: STEAM_ID,
        login_id: 7,
    })
    .unwrap();
}

#[tokio::test]
async fn signing_in_passes_on_each_code_and_keeps_the_sign_in() {
    let steam = FakeSteam::start().await;
    steam.qr_goes(vec![QrScript {
        scanned_at: Some(1),
        approved_at: Some(2),
        ..Default::default()
    }]);
    let (repo, _) = repository(&steam, "link");
    let (tx, mut rx) = mpsc::unbounded_channel();

    repo.link(tx).await.unwrap();

    let mut shown = Vec::new();
    while let Ok(c) = rx.try_recv() {
        shown.push(c);
    }
    assert_eq!(
        shown,
        [
            LoginChallenge {
                url: "https://s.team/q/1/1".into(),
                scanned: false,
            },
            LoginChallenge {
                url: "https://s.team/q/1/1".into(),
                scanned: true,
            },
        ]
    );
    assert!(repo.is_linked());
    assert_eq!(repo.name().as_deref(), Some(ACCOUNT));
    assert!(!repo.is_rejected());
}

#[tokio::test]
async fn a_sign_in_steam_turns_down_is_expired() {
    let steam = FakeSteam::start().await;
    steam.refuse_logon(EResult::ACCESS_DENIED);
    let (repo, file) = repository(&steam, "rejected");
    saved(&file, 4_000_000_000);

    repo.verify().await;

    assert!(repo.is_linked() && repo.is_rejected());
}

#[tokio::test]
async fn a_sign_in_past_its_date_is_expired_without_asking() {
    let steam = FakeSteam::start().await;
    let (repo, file) = repository(&steam, "old");
    saved(&file, 1_000_000_000);
    assert!(repo.is_rejected());
}

#[tokio::test]
async fn signing_out_forgets_the_sign_in_and_tells_steam() {
    let steam = FakeSteam::start().await;
    let (repo, file) = repository(&steam, "unlink");
    saved(&file, 4_000_000_000);
    let refresh = file.credentials().unwrap().refresh_token;

    repo.unlink().unwrap();

    assert!(!repo.is_linked());
    for _ in 0..200 {
        if !steam.revoked().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(steam.revoked(), [refresh]);
}
