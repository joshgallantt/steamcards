//! Where the preferences domain meets its data layer: the preferences kept in
//! the config file, through one repository handed to every use case. The
//! composition root names the file.

use std::sync::Arc;

use config_file::ConfigFile;
use preferences::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetAutoUpdateUseCase,
    DefaultSetGameTierUseCase, DefaultSetHoursBeforeDropsUseCase, DefaultSetOnlyPriorityUseCase,
    DefaultSetRestartGamesUseCase, DefaultSetSkipPrivateUseCase, DefaultSetSkipRefundableUseCase,
    GetPreferencesUseCase, PreferencesRepository, SetAppearOnlineUseCase, SetAutoUpdateUseCase,
    SetGameTierUseCase, SetHoursBeforeDropsUseCase, SetOnlyPriorityUseCase, SetRestartGamesUseCase,
    SetSkipPrivateUseCase, SetSkipRefundableUseCase,
};
use preferences_data::{DefaultPreferencesRepository, FilePreferencesStore, PreferencesStore};

pub struct PreferencesComponent {
    pub get_preferences: Arc<dyn GetPreferencesUseCase>,
    pub set_game_tier: Arc<dyn SetGameTierUseCase>,
    pub set_only_priority: Arc<dyn SetOnlyPriorityUseCase>,
    pub set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
    pub set_restart_games: Arc<dyn SetRestartGamesUseCase>,
    pub set_auto_update: Arc<dyn SetAutoUpdateUseCase>,
    pub set_hours_before_drops: Arc<dyn SetHoursBeforeDropsUseCase>,
    pub set_skip_private: Arc<dyn SetSkipPrivateUseCase>,
    pub set_skip_refundable: Arc<dyn SetSkipRefundableUseCase>,
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
            set_appear_online: Arc::new(DefaultSetAppearOnlineUseCase::new(repo.clone())),
            set_restart_games: Arc::new(DefaultSetRestartGamesUseCase::new(repo.clone())),
            set_auto_update: Arc::new(DefaultSetAutoUpdateUseCase::new(repo.clone())),
            set_hours_before_drops: Arc::new(DefaultSetHoursBeforeDropsUseCase::new(repo.clone())),
            set_skip_private: Arc::new(DefaultSetSkipPrivateUseCase::new(repo.clone())),
            set_skip_refundable: Arc::new(DefaultSetSkipRefundableUseCase::new(repo)),
        }
    }
}
