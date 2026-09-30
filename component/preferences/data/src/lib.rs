//! The preferences domain's contract, satisfied by the config file. Imports
//! `preferences` because the contract is declared there; `preferences`
//! imports nothing back.

use std::sync::Arc;

use config_file::{ConfigFile, StoredPreferences};
use game::AppId;
use preferences::{Preferences, PreferencesRepository};

pub struct FilePreferencesRepository {
    file: Arc<ConfigFile>,
}

impl FilePreferencesRepository {
    pub fn new(file: Arc<ConfigFile>) -> Self {
        Self { file }
    }
}

impl PreferencesRepository for FilePreferencesRepository {
    fn preferences(&self) -> Preferences {
        let s = self.file.preferences();
        Preferences {
            priority_games: s.priority_games.into_iter().map(AppId).collect(),
            skipped_games: s.skipped_games.into_iter().map(AppId).collect(),
            only_priority: s.only_priority,
            appear_online: s.appear_online,
        }
    }

    fn save(&self, p: Preferences) -> anyhow::Result<()> {
        self.file.save_preferences(StoredPreferences {
            priority_games: p.priority_games.iter().map(|g| g.0).collect(),
            skipped_games: p.skipped_games.iter().map(|g| g.0).collect(),
            only_priority: p.only_priority,
            appear_online: p.appear_online,
        })
    }
}
