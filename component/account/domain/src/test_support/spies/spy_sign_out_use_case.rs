use std::sync::atomic::{AtomicUsize, Ordering};

use crate::{SignOutError, SignOutUseCase};

/// Signs out of nothing, successfully, counting how often it was asked.
#[derive(Default)]
pub struct SpySignOutUseCase {
    sign_outs: AtomicUsize,
}

impl SpySignOutUseCase {
    pub fn sign_outs(&self) -> usize {
        self.sign_outs.load(Ordering::Relaxed)
    }
}

impl SignOutUseCase for SpySignOutUseCase {
    fn call(&self) -> Result<(), SignOutError> {
        self.sign_outs.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}
