use std::sync::Mutex;

use crate::{GetPreferencesUseCase, Preferences};

/// Answers with whatever preferences were last set, so a test can change
/// them as it goes.
#[derive(Default)]
pub struct StubGetPreferencesUseCase {
    preferences: Mutex<Preferences>,
}

impl StubGetPreferencesUseCase {
    pub fn new(preferences: Preferences) -> Self {
        Self {
            preferences: Mutex::new(preferences),
        }
    }

    pub fn set(&self, preferences: Preferences) {
        *self.preferences.lock().unwrap() = preferences;
    }
}

impl GetPreferencesUseCase for StubGetPreferencesUseCase {
    fn call(&self) -> Preferences {
        self.preferences.lock().unwrap().clone()
    }
}
