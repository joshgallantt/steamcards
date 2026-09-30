use std::sync::Arc;

use crate::{AccountRepository, CheckSignInUseCase};

pub struct DefaultCheckSignInUseCase {
    repo: Arc<dyn AccountRepository>,
}

impl DefaultCheckSignInUseCase {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl CheckSignInUseCase for DefaultCheckSignInUseCase {
    fn call(&self) {
        if self.repo.is_linked() {
            let repo = Arc::clone(&self.repo);
            tokio::spawn(async move { repo.verify().await });
        }
    }
}
