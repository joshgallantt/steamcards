//! Doubles for other crates' tests. A double stands in for the contract, so
//! no test here or downstream needs a disk.

mod stubs;

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{Preferences, PreferencesRepository};

pub use stubs::StubGetPreferencesUseCase;

/// Keeps preferences in memory. `failing()` makes every save err, so the
/// "didn't stick" path can be driven.
#[derive(Default)]
pub struct InMemoryPreferencesRepository {
    preferences: Mutex<Preferences>,
    fail: AtomicBool,
}

impl InMemoryPreferencesRepository {
    pub fn new(preferences: Preferences) -> Self {
        Self {
            preferences: Mutex::new(preferences),
            fail: AtomicBool::new(false),
        }
    }

    pub fn failing(self) -> Self {
        self.fail.store(true, Ordering::Relaxed);
        self
    }
}

impl PreferencesRepository for InMemoryPreferencesRepository {
    fn preferences(&self) -> Preferences {
        self.preferences.lock().unwrap().clone()
    }

    fn save(&self, p: Preferences) -> anyhow::Result<()> {
        if self.fail.load(Ordering::Relaxed) {
            anyhow::bail!("disk full");
        }
        *self.preferences.lock().unwrap() = p;
        Ok(())
    }
}
