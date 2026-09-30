use std::sync::Arc;

use account::LoginChallenge;
use async_trait::async_trait;
use steam_api::{SteamClient, auth};
use tokio::sync::mpsc;

/// The saved sign-in, and Steam's say on it.
#[async_trait]
pub trait AccountClient: Send + Sync {
    /// The account name of the sign-in saved, if one is: empty when Steam
    /// didn't say it.
    fn signed_in_as(&self) -> Option<String>;

    /// Whether Steam has turned the saved sign-in down, or it has run out.
    fn is_rejected(&self) -> bool;

    /// Signs on with the saved sign-in, so Steam says whether it still takes
    /// it. A sign-on that couldn't reach Steam changes nothing.
    async fn check(&self);

    /// Signs in with the Steam app: each code to show as it comes, then the
    /// sign-in is kept. Errs with a reason fit to show the user.
    async fn sign_in(
        &self,
        challenges: mpsc::UnboundedSender<LoginChallenge>,
    ) -> anyhow::Result<()>;

    /// Forgets the saved sign-in. Steam is told to end it too, in the
    /// background.
    fn sign_out(&self) -> anyhow::Result<()>;
}

/// The sign-in the Steam client holds.
pub struct SteamAccountClient {
    steam: Arc<SteamClient>,
}

impl SteamAccountClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self { steam }
    }
}

#[async_trait]
impl AccountClient for SteamAccountClient {
    fn signed_in_as(&self) -> Option<String> {
        self.steam.credentials().map(|c| c.account_name)
    }

    fn is_rejected(&self) -> bool {
        self.steam.is_rejected() || self.steam.has_expired()
    }

    /// Steam says whether it takes the sign-in when a session signs on with
    /// it, so checking is signing on: the farmer uses the same connection.
    async fn check(&self) {
        if let Err(e) = self.steam.connection().await {
            self.steam.log().line(&format!("checking the sign-in: {e}"));
        }
    }

    async fn sign_in(
        &self,
        challenges: mpsc::UnboundedSender<LoginChallenge>,
    ) -> anyhow::Result<()> {
        let conn = self.steam.open().await?;
        let approved = auth::sign_in_with_qr(&conn, |qr| {
            let _ = challenges.send(LoginChallenge {
                url: qr.url,
                scanned: qr.scanned,
            });
        })
        .await?;
        self.steam.save(&approved).await
    }

    fn sign_out(&self) -> anyhow::Result<()> {
        self.steam.forget()
    }
}
