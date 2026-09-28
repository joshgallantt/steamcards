//! Doubles for other crates' tests.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{
    Account, AccountRepository, GetAccount, LinkAccount, LinkError, LoginChallenge, RefreshAccount,
    UnlinkAccount,
};

/// Answers with a fixed account, or nobody.
pub fn fixed_account(account: Option<Account>) -> GetAccount {
    Arc::new(move || account.clone())
}

/// Does nothing when asked to refresh.
pub fn no_refresh() -> RefreshAccount {
    Arc::new(|| {})
}

/// Refuses every sign-in, so nothing reaches Steam.
pub fn refusing_link() -> LinkAccount {
    Arc::new(|_| tokio::spawn(async { Err(LinkError::Refused("not in a test".into())) }))
}

/// Signs out of nothing, successfully, counting how often it was asked.
pub fn recording_unlink() -> (UnlinkAccount, Arc<AtomicUsize>) {
    let asked = Arc::new(AtomicUsize::new(0));
    let count = asked.clone();
    let unlink: UnlinkAccount = Arc::new(move || {
        count.fetch_add(1, Ordering::Relaxed);
        Ok(())
    });
    (unlink, asked)
}

/// A saved sign-in, held in memory.
pub struct InMemoryAccountRepository {
    pub name: Mutex<Option<String>>,
    pub linked: AtomicBool,
    pub rejected: AtomicBool,
    /// What the next `verify` finds: `Some(accepted)`, or `None` for
    /// "couldn't reach Steam".
    pub verifies_as: Mutex<Option<bool>>,
    /// What the next `link` does: sends these challenges, then succeeds with
    /// the name or fails with the reason.
    pub links_as: Mutex<(Vec<LoginChallenge>, Result<String, String>)>,
    /// The next `unlink` fails, keeping the sign-in.
    pub unlink_fails: AtomicBool,
}

impl InMemoryAccountRepository {
    pub fn signed_out() -> Self {
        Self {
            name: Mutex::new(None),
            linked: AtomicBool::new(false),
            rejected: AtomicBool::new(false),
            verifies_as: Mutex::new(None),
            links_as: Mutex::new((Vec::new(), Err("not set up".into()))),
            unlink_fails: AtomicBool::new(false),
        }
    }

    pub fn signed_in(name: &str) -> Self {
        let repo = Self::signed_out();
        repo.linked.store(true, Ordering::Relaxed);
        *repo.name.lock().unwrap() = Some(name.to_owned()).filter(|n| !n.is_empty());
        repo
    }
}

#[async_trait]
impl AccountRepository for InMemoryAccountRepository {
    fn is_linked(&self) -> bool {
        self.linked.load(Ordering::Relaxed)
    }

    fn name(&self) -> Option<String> {
        self.name.lock().unwrap().clone()
    }

    fn is_rejected(&self) -> bool {
        self.rejected.load(Ordering::Relaxed)
    }

    async fn verify(&self) {
        if let Some(accepted) = *self.verifies_as.lock().unwrap() {
            self.rejected.store(!accepted, Ordering::Relaxed);
        }
    }

    async fn link(&self, challenges: mpsc::UnboundedSender<LoginChallenge>) -> anyhow::Result<()> {
        let (shown, outcome) = self.links_as.lock().unwrap().clone();
        for c in shown {
            let _ = challenges.send(c);
        }
        let name = outcome.map_err(anyhow::Error::msg)?;
        *self.name.lock().unwrap() = Some(name);
        self.linked.store(true, Ordering::Relaxed);
        self.rejected.store(false, Ordering::Relaxed);
        Ok(())
    }

    fn unlink(&self) -> anyhow::Result<()> {
        if self.unlink_fails.load(Ordering::Relaxed) {
            anyhow::bail!("disk full");
        }
        self.linked.store(false, Ordering::Relaxed);
        self.rejected.store(false, Ordering::Relaxed);
        *self.name.lock().unwrap() = None;
        Ok(())
    }
}
