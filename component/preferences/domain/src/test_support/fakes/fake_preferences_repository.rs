use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{Preferences, PreferencesRepository};

/// Keeps preferences in memory. `failing()` makes every save err, so the
/// "didn't stick" path can be driven.
#[derive(Default)]
pub struct FakePreferencesRepository {
    preferences: Mutex<Preferences>,
    fail: AtomicBool,
}

impl FakePreferencesRepository {
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

impl PreferencesRepository for FakePreferencesRepository {
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
