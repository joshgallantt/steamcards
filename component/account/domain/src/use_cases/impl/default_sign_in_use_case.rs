use std::sync::Arc;

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{AccountRepository, LoginChallenge, SignInError, SignInUseCase};

pub struct DefaultSignInUseCase {
    repo: Arc<dyn AccountRepository>,
}

impl DefaultSignInUseCase {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl SignInUseCase for DefaultSignInUseCase {
    fn call(
        &self,
        challenges: mpsc::UnboundedSender<LoginChallenge>,
    ) -> JoinHandle<Result<(), SignInError>> {
        let repo = Arc::clone(&self.repo);
        tokio::spawn(async move {
            repo.link(challenges)
                .await
                .map_err(|e| SignInError::Refused(e.to_string()))
        })
    }
}
