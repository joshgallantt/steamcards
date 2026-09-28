//! Signing in with a QR code, against a stand-in for Steam.

use std::sync::{Arc, Mutex};

use config_file::ConfigFile;
use debug_log::DebugLog;
use steam_api::{
    Session,
    auth::{self, QrCode},
    cm::Connection,
    test_support::{ACCOUNT, FakeSteam, QrScript, STEAM_ID},
};

async fn connected(steam: &FakeSteam) -> Connection {
    Connection::connect(steam.url(), &DebugLog::off())
        .await
        .unwrap()
}

/// Every QR code shown, in order.
fn shown() -> (Arc<Mutex<Vec<QrCode>>>, impl FnMut(QrCode)) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    (seen, move |qr| log.lock().unwrap().push(qr))
}

fn session(steam: &FakeSteam, name: &str) -> Session {
    let dir = std::env::temp_dir().join(format!("steamcards-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Arc::new(ConfigFile::open(dir.join("config.json")).unwrap());
    Session::with_endpoints(store, &DebugLog::off(), steam.endpoints())
}

#[tokio::test]
async fn a_scanned_and_approved_code_signs_in() {
    let steam = FakeSteam::start().await;
    steam.qr_goes(vec![QrScript {
        new_code_at: Some(1),
        scanned_at: Some(2),
        approved_at: Some(3),
        ..Default::default()
    }]);
    let conn = connected(&steam).await;
    let (seen, show) = shown();

    let approved = auth::sign_in_with_qr(&conn, show).await.unwrap();

    assert_eq!(approved.account_name, ACCOUNT);
    assert_eq!(approved.steam_id, STEAM_ID);
    assert!(!approved.refresh_token.is_empty() && !approved.access_token.is_empty());
    let seen = seen.lock().unwrap();
    let urls: Vec<&str> = seen.iter().map(|q| q.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://s.team/q/1/1",
            "https://s.team/q/1/1-1",
            "https://s.team/q/1/1-1"
        ],
        "a new code replaces the first, and is shown again once scanned"
    );
    let scanned: Vec<bool> = seen.iter().map(|q| q.scanned).collect();
    assert_eq!(scanned, [false, false, true]);
}

#[tokio::test]
async fn a_code_that_expires_unscanned_is_replaced() {
    let steam = FakeSteam::start().await;
    steam.qr_goes(vec![
        QrScript {
            ended_at: Some(2),
            ..Default::default()
        },
        QrScript {
            approved_at: Some(1),
            ..Default::default()
        },
    ]);
    let conn = connected(&steam).await;
    let (seen, show) = shown();

    auth::sign_in_with_qr(&conn, show).await.unwrap();

    assert_eq!(steam.qr_codes_begun(), 2);
    let urls: Vec<String> = seen.lock().unwrap().iter().map(|q| q.url.clone()).collect();
    assert_eq!(urls, ["https://s.team/q/1/1", "https://s.team/q/1/2"]);
}

#[tokio::test]
async fn a_scanned_code_that_isnt_approved_ends_the_sign_in() {
    let steam = FakeSteam::start().await;
    steam.qr_goes(vec![QrScript {
        scanned_at: Some(1),
        ended_at: Some(2),
        ..Default::default()
    }]);
    let conn = connected(&steam).await;
    let (_, show) = shown();

    let e = auth::sign_in_with_qr(&conn, show).await.unwrap_err();

    assert!(e.to_string().contains("wasn't approved"), "{e}");
    assert_eq!(steam.qr_codes_begun(), 1, "no new code after a no");
}

#[tokio::test]
async fn a_sign_in_is_kept_and_signs_on() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "keep");

    let conn = session.open().await.unwrap();
    let approved = auth::sign_in_with_qr(&conn, |_| {}).await.unwrap();
    session.save(&approved).await.unwrap();

    let saved = session.credentials().unwrap();
    assert_eq!(saved.account_name, ACCOUNT);
    assert_eq!(saved.steam_id, STEAM_ID);
    assert_ne!(saved.login_id, 0, "a login ID of its own");
    let signed_on = session.connection().await.unwrap();
    assert!(signed_on.is_signed_on());
    assert_eq!(steam.logons(), [approved.refresh_token]);
}

#[tokio::test]
async fn signing_in_again_keeps_the_login_id() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "relink");
    for _ in 0..2 {
        let conn = session.open().await.unwrap();
        let approved = auth::sign_in_with_qr(&conn, |_| {}).await.unwrap();
        let before = session.credentials().map(|c| c.login_id);
        session.save(&approved).await.unwrap();
        if let Some(before) = before {
            assert_eq!(session.credentials().unwrap().login_id, before);
        }
    }
}

#[tokio::test]
async fn signing_out_ends_the_sign_in_at_steam() {
    let steam = FakeSteam::start().await;
    let session = session(&steam, "forget");
    let conn = session.open().await.unwrap();
    let approved = auth::sign_in_with_qr(&conn, |_| {}).await.unwrap();
    session.save(&approved).await.unwrap();
    session.connection().await.unwrap();

    session.forget().unwrap();

    assert!(session.credentials().is_none(), "forgotten at once");
    for _ in 0..200 {
        if !steam.revoked().is_empty() && steam.log_offs() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(steam.revoked(), [approved.refresh_token]);
    assert_eq!(steam.log_offs(), 1);
}
