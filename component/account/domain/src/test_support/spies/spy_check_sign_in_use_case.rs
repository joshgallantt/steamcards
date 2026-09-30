use std::sync::atomic::{AtomicUsize, Ordering};

use crate::CheckSignInUseCase;

/// Checks nothing, counting how often it was asked to.
#[derive(Default)]
pub struct SpyCheckSignInUseCase {
    checks: AtomicUsize,
}

impl SpyCheckSignInUseCase {
    pub fn checks(&self) -> usize {
        self.checks.load(Ordering::Relaxed)
    }
}

impl CheckSignInUseCase for SpyCheckSignInUseCase {
    fn call(&self) {
        self.checks.fetch_add(1, Ordering::Relaxed);
    }
}
