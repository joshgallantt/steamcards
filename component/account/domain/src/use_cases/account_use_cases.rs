//! One function per thing the user does with their account. Each use case
//! is a value: its type says what it takes and gives, and a constructor
//! builds the real one over the repository. A view model holds only the
//! ones it calls, and a test double is just a closure.

use std::sync::Arc;

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{Account, AccountRepository, LinkError, LoginChallenge, UnlinkError};

/// The signed-in account, or `None` when nobody is signed in.
pub type GetAccount = Arc<dyn Fn() -> Option<Account> + Send + Sync>;

/// Re-checks the saved sign-in in the background; `GetAccount` reflects the
/// result once it lands.
pub type RefreshAccount = Arc<dyn Fn() + Send + Sync>;

/// Starts signing in. Challenges arrive on the channel; the handle resolves
/// when the sign-in ends, and aborting it stops the sign-in.
pub type LinkAccount = Arc<
    dyn Fn(mpsc::UnboundedSender<LoginChallenge>) -> JoinHandle<Result<(), LinkError>>
        + Send
        + Sync,
>;

/// Signs out: the saved sign-in is forgotten, so nothing farms until the
/// account signs in again.
pub type UnlinkAccount = Arc<dyn Fn() -> Result<(), UnlinkError> + Send + Sync>;

pub fn get_account(repo: Arc<dyn AccountRepository>) -> GetAccount {
    Arc::new(move || {
        repo.is_linked().then(|| Account {
            name: repo.name().unwrap_or_default(),
            expired: repo.is_rejected(),
        })
    })
}

pub fn refresh_account(repo: Arc<dyn AccountRepository>) -> RefreshAccount {
    Arc::new(move || {
        if repo.is_linked() {
            let repo = Arc::clone(&repo);
            tokio::spawn(async move { repo.verify().await });
        }
    })
}

pub fn link_account(repo: Arc<dyn AccountRepository>) -> LinkAccount {
    Arc::new(move |challenges| {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            repo.link(challenges)
                .await
                .map_err(|e| LinkError::Refused(e.to_string()))
        })
    })
}

pub fn unlink_account(repo: Arc<dyn AccountRepository>) -> UnlinkAccount {
    Arc::new(move || repo.unlink().map_err(|_| UnlinkError::Unavailable))
}
