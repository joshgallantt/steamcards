mod r#impl;
mod preferences_use_cases;

pub use r#impl::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetAutoUpdateUseCase,
    DefaultSetGameTierUseCase, DefaultSetOnlyPriorityUseCase, DefaultSetRestartGamesUseCase,
};
pub use preferences_use_cases::{
    GetPreferencesUseCase, SetAppearOnlineUseCase, SetAutoUpdateUseCase, SetGameTierUseCase,
    SetOnlyPriorityUseCase, SetRestartGamesUseCase,
};
