use std::sync::atomic::{AtomicUsize, Ordering};

use tokio::task::JoinHandle;

use crate::{GameError, ReadLibraryUseCase, SteamLibrary};

/// Answers every read with `result`, counting the reads.
pub struct SpyReadLibraryUseCase {
    result: Result<SteamLibrary, GameError>,
    reads: AtomicUsize,
}

impl SpyReadLibraryUseCase {
    pub fn answering(result: Result<SteamLibrary, GameError>) -> Self {
        Self {
            result,
            reads: AtomicUsize::new(0),
        }
    }

    pub fn reads(&self) -> usize {
        self.reads.load(Ordering::Relaxed)
    }
}

impl ReadLibraryUseCase for SpyReadLibraryUseCase {
    fn call(&self) -> JoinHandle<Result<SteamLibrary, GameError>> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        let result = self.result.clone();
        tokio::spawn(async move { result })
    }
}
