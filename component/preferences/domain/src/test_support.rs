//! Doubles for other crates' tests. A double stands in for the contract, so
//! no test here or downstream needs a disk.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::{GetPreferences, Preferences, PreferencesRepository};

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

/// Preferences a test can change as it goes: `get()` answers with whatever
/// was last `set`.
#[derive(Clone, Default)]
pub struct ChangingPreferences(Arc<Mutex<Preferences>>);

impl ChangingPreferences {
    pub fn new(p: Preferences) -> Self {
        Self(Arc::new(Mutex::new(p)))
    }

    pub fn set(&self, p: Preferences) {
        *self.0.lock().unwrap() = p;
    }

    pub fn get(&self) -> GetPreferences {
        let current = self.0.clone();
        Arc::new(move || current.lock().unwrap().clone())
    }
}
