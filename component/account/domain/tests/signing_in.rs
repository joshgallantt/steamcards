//! Acceptance tier: signing in and out as the user meets it.

use std::sync::{Arc, atomic::Ordering};

use account::{
    Account, CheckSignInUseCase, DefaultCheckSignInUseCase, DefaultGetAccountUseCase,
    DefaultSignInUseCase, DefaultSignOutUseCase, GetAccountUseCase, LoginChallenge, SignInError,
    SignInUseCase, SignOutError, SignOutUseCase, test_support::InMemoryAccountRepository,
};
use tokio::sync::mpsc;

struct Player {
    steam: Arc<InMemoryAccountRepository>,
    get_account: Arc<dyn GetAccountUseCase>,
    check_sign_in: Arc<dyn CheckSignInUseCase>,
    sign_in: Arc<dyn SignInUseCase>,
    sign_out: Arc<dyn SignOutUseCase>,
}

impl Player {
    fn with(steam: InMemoryAccountRepository) -> Self {
        let steam = Arc::new(steam);
        Self {
            get_account: Arc::new(DefaultGetAccountUseCase::new(steam.clone())),
            check_sign_in: Arc::new(DefaultCheckSignInUseCase::new(steam.clone())),
            sign_in: Arc::new(DefaultSignInUseCase::new(steam.clone())),
            sign_out: Arc::new(DefaultSignOutUseCase::new(steam.clone())),
            steam,
        }
    }

    fn sees(&self) -> Option<Account> {
        self.get_account.call()
    }

    async fn signs_in(&self) -> (Vec<LoginChallenge>, Result<(), SignInError>) {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let outcome = self.sign_in.call(tx).await.unwrap();
        let mut seen = Vec::new();
        while let Ok(c) = rx.try_recv() {
            seen.push(c);
        }
        (seen, outcome)
    }
}

fn account(name: &str, expired: bool) -> Option<Account> {
    Some(Account {
        name: name.into(),
        expired,
    })
}

#[test]
fn nobody_is_signed_in_until_someone_does() {
    let player = Player::with(InMemoryAccountRepository::signed_out());
    assert_eq!(player.sees(), None);
}

#[tokio::test]
async fn signing_in_shows_the_codes_then_the_account() {
    let player = Player::with(InMemoryAccountRepository::signed_out());
    let codes = vec![
        LoginChallenge {
            url: "https://s.team/q/1/1".into(),
            scanned: false,
        },
        LoginChallenge {
            url: "https://s.team/q/1/1".into(),
            scanned: true,
        },
    ];
    *player.steam.links_as.lock().unwrap() = (codes.clone(), Ok("cardfarmer".into()));

    let (seen, outcome) = player.signs_in().await;

    assert_eq!(seen, codes);
    assert_eq!(outcome, Ok(()));
    assert_eq!(player.sees(), account("cardfarmer", false));
}

#[tokio::test]
async fn a_sign_in_that_fails_says_why() {
    let player = Player::with(InMemoryAccountRepository::signed_out());
    *player.steam.links_as.lock().unwrap() = (
        Vec::new(),
        Err("the sign-in wasn't approved in the Steam app — try again".into()),
    );

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
async fn a_rejected_sign_in_shows_as_expired_until_signed_in_again() {
    let player = Player::with(InMemoryAccountRepository::signed_in("cardfarmer"));
    *player.steam.verifies_as.lock().unwrap() = Some(false);
    player.check_sign_in.call();
    tokio::task::yield_now().await;
    assert_eq!(player.sees(), account("cardfarmer", true));

    *player.steam.links_as.lock().unwrap() = (Vec::new(), Ok("cardfarmer".into()));
    player.signs_in().await.1.unwrap();
    assert_eq!(player.sees(), account("cardfarmer", false));
}

#[tokio::test]
async fn a_check_that_cant_reach_steam_changes_nothing() {
    let player = Player::with(InMemoryAccountRepository::signed_in("cardfarmer"));
    player.steam.rejected.store(true, Ordering::Relaxed);
    player.check_sign_in.call();
    tokio::task::yield_now().await;
    assert_eq!(player.sees(), account("cardfarmer", true));
}

#[test]
fn signing_out_forgets_the_sign_in() {
    let player = Player::with(InMemoryAccountRepository::signed_in("cardfarmer"));
    player.sign_out.call().unwrap();
    assert_eq!(player.sees(), None);
}

#[test]
fn a_sign_out_that_didnt_stick_keeps_the_sign_in() {
    let player = Player::with(InMemoryAccountRepository::signed_in("cardfarmer"));
    player.steam.unlink_fails.store(true, Ordering::Relaxed);
    assert_eq!(player.sign_out.call(), Err(SignOutError::Unavailable));
    assert_eq!(player.sees(), account("cardfarmer", false));
}
