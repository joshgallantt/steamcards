//! The account domain's contract, satisfied by Steam. Imports `account`
//! because the contract is declared there; `account` imports nothing back.

use std::sync::Arc;

use account::{AccountRepository, LoginChallenge};
use async_trait::async_trait;
use steam_api::{Session, auth};
use tokio::sync::mpsc;

/// A Steam sign-in with a QR code the Steam app scans and approves.
pub struct SteamAccountRepository {
    session: Arc<Session>,
}

impl SteamAccountRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self { session }
    }
}

#[async_trait]
impl AccountRepository for SteamAccountRepository {
    fn is_linked(&self) -> bool {
        self.session.credentials().is_some()
    }

    fn name(&self) -> Option<String> {
        self.session
            .credentials()
            .map(|c| c.account_name)
            .filter(|n| !n.is_empty())
    }

    fn is_rejected(&self) -> bool {
        self.session.is_rejected() || self.session.has_expired()
    }

    /// Steam says whether it takes the sign-in when a session signs on with
    /// it, so checking is signing on: the farmer uses the same connection.
    async fn verify(&self) {
        if let Err(e) = self.session.connection().await {
            self.session
                .log()
                .line(&format!("checking the sign-in: {e}"));
        }
    }

    async fn link(&self, challenges: mpsc::UnboundedSender<LoginChallenge>) -> anyhow::Result<()> {
        let conn = self.session.open().await?;
        let approved = auth::sign_in_with_qr(&conn, |qr| {
            let _ = challenges.send(LoginChallenge {
                url: qr.url,
                scanned: qr.scanned,
            });
        })
        .await?;
        self.session.save(&approved).await
    }

    fn unlink(&self) -> anyhow::Result<()> {
        self.session.forget()
    }
}
