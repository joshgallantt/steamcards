use std::sync::atomic::{AtomicUsize, Ordering};

use crate::EndSessionUseCase;

/// Ends nothing, counting the sessions it was asked to end.
#[derive(Default)]
pub struct SpyEndSessionUseCase {
    ended: AtomicUsize,
}

impl SpyEndSessionUseCase {
    pub fn ended(&self) -> usize {
        self.ended.load(Ordering::Relaxed)
    }
}

impl EndSessionUseCase for SpyEndSessionUseCase {
    fn call(&self) {
        self.ended.fetch_add(1, Ordering::Relaxed);
    }
}
