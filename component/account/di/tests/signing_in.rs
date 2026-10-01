//! Acceptance tier: signing in and out as the user meets it, through the
//! account component as steamcards wires it, against a stand-in for Steam.

mod support;

use std::time::Duration;

use account::{Account, LoginChallenge, SignInError, SignOutError};
use steam_api::{
    EResult,
    test_support::{ACCOUNT, QrScript},
};
use support::{Player, eventually};

fn signed_in(expired: bool) -> Option<Account> {
    Some(Account {
        name: ACCOUNT.into(),
        expired,
    })
}

/// The Steam app scans the code, and the sign-in is approved.
fn approved() -> Vec<QrScript> {
    vec![QrScript {
        scanned_at: Some(1),
        approved_at: Some(2),
        ..Default::default()
    }]
}

#[tokio::test]
async fn nobody_is_signed_in_until_someone_does() {
    let player = Player::new("nobody").await;
    assert_eq!(player.sees(), None);
}

#[tokio::test]
async fn signing_in_shows_the_codes_then_the_account() {
    let player = Player::new("sign-in").await;
    player.steam.qr_goes(approved());

    let (seen, outcome) = player.signs_in().await;

    let code = |scanned| LoginChallenge {
        url: "https://s.team/q/1/1".into(),
        scanned,
    };
    assert_eq!(seen, [code(false), code(true)]);
    assert_eq!(outcome, Ok(()));
    assert_eq!(player.sees(), signed_in(false));
}

#[tokio::test]
async fn a_sign_in_thats_kept_is_there_when_steamcards_starts_again() {
    let player = Player::new("kept").await;
    player.steam.qr_goes(approved());
    player.signs_in().await.1.unwrap();

    let player = player.comes_back();

    assert_eq!(player.sees(), signed_in(false));
}

#[tokio::test]
async fn a_sign_in_that_isnt_approved_says_why() {
    let player = Player::new("not-approved").await;
    player.steam.qr_goes(vec![QrScript {
        scanned_at: Some(1),
        ended_at: Some(2),
        ..Default::default()
    }]);

    let (_, outcome) = player.signs_in().await;

    assert_eq!(
        outcome,
        Err(SignInError::Refused(
            "the sign-in wasn't approved in the Steam app — try again".into()
        ))
    );
    assert_eq!(player.sees(), None);
}

#[tokio::test]
async fn a_sign_in_steam_turns_down_shows_as_expired_until_signed_in_again() {
    let player = Player::signed_in("rejected").await;
    player.steam.refuse_logon(EResult::ACCESS_DENIED);

    player.checks_the_sign_in().await;

    eventually(|| player.sees() == signed_in(true)).await;
    player.steam.qr_goes(approved());
    player.signs_in().await.1.unwrap();
    assert_eq!(player.sees(), signed_in(false));
}

#[tokio::test]
async fn a_check_that_cant_reach_steam_changes_nothing() {
    let player = Player::signed_in("unreached").await;
    player.steam.refuse_logon(EResult::TRY_ANOTHER_CM);

    player.checks_the_sign_in().await;

    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(player.sees(), signed_in(false));
}

#[tokio::test]
async fn signing_out_forgets_the_sign_in() {
    let player = Player::signed_in("sign-out").await;

    player.signs_out().unwrap();

    assert_eq!(player.sees(), None);
    assert_eq!(player.comes_back().sees(), None, "after a restart too");
}

#[tokio::test]
async fn a_sign_out_that_didnt_stick_keeps_the_sign_in() {
    let player = Player::signed_in("full-disk").await;
    player.runs_out_of_disk();

    assert_eq!(player.signs_out(), Err(SignOutError::Unavailable));

    assert_eq!(player.sees(), signed_in(false));
}
