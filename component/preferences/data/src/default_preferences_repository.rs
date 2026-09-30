use std::sync::Arc;

use preferences::{Preferences, PreferencesRepository};

use crate::PreferencesStore;

/// The preferences, as the store keeps them.
pub struct DefaultPreferencesRepository {
    store: Arc<dyn PreferencesStore>,
}

impl DefaultPreferencesRepository {
    pub fn new(store: Arc<dyn PreferencesStore>) -> Self {
        Self { store }
    }
}

impl PreferencesRepository for DefaultPreferencesRepository {
    fn preferences(&self) -> Preferences {
        self.store.preferences()
    }

    fn save(&self, preferences: Preferences) -> anyhow::Result<()> {
        self.store.save(&preferences)
    }
}
