//! Where the preferences domain meets its data layer: one repository over the
//! config file, handed to every use case. The composition root names the file.

use std::sync::Arc;

use config_file::ConfigFile;
use preferences::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetGameTierUseCase,
    DefaultSetOnlyPriorityUseCase, GetPreferencesUseCase, PreferencesRepository,
    SetAppearOnlineUseCase, SetGameTierUseCase, SetOnlyPriorityUseCase,
};
use preferences_data::FilePreferencesRepository;

pub struct PreferencesComponent {
    pub get_preferences: Arc<dyn GetPreferencesUseCase>,
    pub set_game_tier: Arc<dyn SetGameTierUseCase>,
    pub set_only_priority: Arc<dyn SetOnlyPriorityUseCase>,
    pub set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
}

impl PreferencesComponent {
    pub fn new(file: Arc<ConfigFile>) -> Self {
        Self::over(Arc::new(FilePreferencesRepository::new(file)))
    }

    pub fn over(repo: Arc<dyn PreferencesRepository>) -> Self {
        Self {
            get_preferences: Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            set_game_tier: Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            set_only_priority: Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            set_appear_online: Arc::new(DefaultSetAppearOnlineUseCase::new(repo)),
        }
    }
}
