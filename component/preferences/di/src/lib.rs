//! Where the preferences domain meets its data layer: one repository over the
//! config file, handed to every use case. The composition root names the file.

use std::sync::Arc;

use config_file::ConfigFile;
use preferences::{
    GetPreferences, PreferencesRepository, SetAppearOnline, SetGameTier, SetOnlyPriority,
    get_preferences, set_appear_online, set_game_tier, set_only_priority,
};
use preferences_data::FilePreferencesRepository;

pub struct PreferencesComponent {
    pub get: GetPreferences,
    pub set_game_tier: SetGameTier,
    pub set_only_priority: SetOnlyPriority,
    pub set_appear_online: SetAppearOnline,
}

impl PreferencesComponent {
    pub fn new(file: Arc<ConfigFile>) -> Self {
        Self::over(Arc::new(FilePreferencesRepository::new(file)))
    }

    pub fn over(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self {
            get: get_preferences(repo.clone()),
            set_game_tier: set_game_tier(repo.clone()),
            set_only_priority: set_only_priority(repo.clone()),
            set_appear_online: set_appear_online(repo),
        }
    }
}
