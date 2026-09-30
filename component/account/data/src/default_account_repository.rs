use std::sync::Arc;

use account::{AccountRepository, LoginChallenge};
use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::AccountClient;

/// The saved sign-in, as the client holds it.
pub struct DefaultAccountRepository {
    client: Arc<dyn AccountClient>,
}

impl DefaultAccountRepository {
    pub fn new(client: Arc<dyn AccountClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AccountRepository for DefaultAccountRepository {
    fn is_linked(&self) -> bool {
        self.client.signed_in_as().is_some()
    }

    fn name(&self) -> Option<String> {
        self.client.signed_in_as().filter(|n| !n.is_empty())
    }

    fn is_rejected(&self) -> bool {
        self.client.is_rejected()
    }

    async fn verify(&self) {
        self.client.check().await;
    }

    async fn link(&self, challenges: mpsc::UnboundedSender<LoginChallenge>) -> anyhow::Result<()> {
        self.client.sign_in(challenges).await
    }

    fn unlink(&self) -> anyhow::Result<()> {
        self.client.sign_out()
    }
}
