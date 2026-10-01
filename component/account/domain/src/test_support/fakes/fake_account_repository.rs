use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{AccountRepository, LoginChallenge, Wallet};

/// A saved sign-in, held in memory.
pub struct FakeAccountRepository {
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
    /// The wallet Steam last said; `None` until it has.
    pub wallet: Mutex<Option<Wallet>>,
}

impl FakeAccountRepository {
    pub fn signed_out() -> Self {
        Self {
            name: Mutex::new(None),
            linked: AtomicBool::new(false),
            rejected: AtomicBool::new(false),
            verifies_as: Mutex::new(None),
            links_as: Mutex::new((Vec::new(), Err("not set up".into()))),
            unlink_fails: AtomicBool::new(false),
            wallet: Mutex::new(None),
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
impl AccountRepository for FakeAccountRepository {
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

    fn wallet(&self) -> Option<Wallet> {
        *self.wallet.lock().unwrap()
    }
}
