//! Where the preferences domain meets its data layer: the preferences kept in
//! the config file, through one repository handed to every use case. The
//! composition root names the file.

use std::sync::Arc;

use config_file::ConfigFile;
use preferences::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetGameTierUseCase,
    DefaultSetOnlyPriorityUseCase, GetPreferencesUseCase, PreferencesRepository,
    SetAppearOnlineUseCase, SetGameTierUseCase, SetOnlyPriorityUseCase,
};
use preferences_data::{DefaultPreferencesRepository, FilePreferencesStore, PreferencesStore};

pub struct PreferencesComponent {
    pub get_preferences: Arc<dyn GetPreferencesUseCase>,
    pub set_game_tier: Arc<dyn SetGameTierUseCase>,
    pub set_only_priority: Arc<dyn SetOnlyPriorityUseCase>,
    pub set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
}

impl PreferencesComponent {
    pub fn new(file: Arc<ConfigFile>) -> Self {
        Self::over(Arc::new(FilePreferencesStore::new(file)))
    }

    /// Over a store of its own: the repository is built here, and never let
    /// out.
    pub fn over(store: Arc<dyn PreferencesStore>) -> Self {
        let repo: Arc<dyn PreferencesRepository> =
            Arc::new(DefaultPreferencesRepository::new(store));
        Self {
            get_preferences: Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            set_game_tier: Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            set_only_priority: Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            set_appear_online: Arc::new(DefaultSetAppearOnlineUseCase::new(repo)),
        }
    }
}
