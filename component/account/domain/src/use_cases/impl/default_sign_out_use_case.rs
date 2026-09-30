use std::sync::Arc;

use crate::{AccountRepository, SignOutError, SignOutUseCase};

pub struct DefaultSignOutUseCase {
    repo: Arc<dyn AccountRepository>,
}

impl DefaultSignOutUseCase {
    pub fn new(repo: Arc<dyn AccountRepository>) -> Self {
        Self { repo }
    }
}

impl SignOutUseCase for DefaultSignOutUseCase {
    fn call(&self) -> Result<(), SignOutError> {
        self.repo.unlink().map_err(|_| SignOutError::Unavailable)
    }
}
