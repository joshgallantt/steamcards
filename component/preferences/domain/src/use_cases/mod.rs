mod r#impl;
mod preferences_use_cases;

pub use r#impl::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetGameTierUseCase,
    DefaultSetOnlyPriorityUseCase,
};
pub use preferences_use_cases::{
    GetPreferencesUseCase, SetAppearOnlineUseCase, SetGameTierUseCase, SetOnlyPriorityUseCase,
};
