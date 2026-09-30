use std::sync::Arc;

use crate::{Account, AccountRepository, GetAccountUseCase};

pub struct DefaultGetAccountUseCase {
    repo: Arc<dyn AccountRepository>,
}

impl DefaultGetAccountUseCase {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl GetAccountUseCase for DefaultGetAccountUseCase {
    fn call(&self) -> Option<Account> {
        self.repo.is_linked().then(|| Account {
            name: self.repo.name().unwrap_or_default(),
            expired: self.repo.is_rejected(),
        })
    }
}
