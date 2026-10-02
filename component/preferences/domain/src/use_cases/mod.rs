mod r#impl;
mod preferences_use_cases;

pub use r#impl::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetAutoUpdateUseCase,
    DefaultSetGameTierUseCase, DefaultSetHoursBeforeDropsUseCase, DefaultSetOnlyPriorityUseCase,
    DefaultSetRestartGamesUseCase, DefaultSetSkipPrivateUseCase, DefaultSetSkipRefundableUseCase,
};
pub use preferences_use_cases::{
    GetPreferencesUseCase, SetAppearOnlineUseCase, SetAutoUpdateUseCase, SetGameTierUseCase,
    SetHoursBeforeDropsUseCase, SetOnlyPriorityUseCase, SetRestartGamesUseCase,
    SetSkipPrivateUseCase, SetSkipRefundableUseCase,
};
