use std::sync::Arc;

use preferences::{
    GetPreferencesUseCase, Preferences, PreferencesError, SetAppearOnlineUseCase,
    SetHoursBeforeDropsUseCase, SetRestartGamesUseCase, SetSkipPrivateUseCase,
    SetSkipRefundableUseCase,
};

/// How farming goes, and how it shows to friends: the hours a game needs
/// before its cards drop, which games are left out, restarting the game,
/// and appearing offline. Keeping steamcards up to date is the update view
/// model's, which starts and stops it.
pub struct SettingsViewModel {
    get: Arc<dyn GetPreferencesUseCase>,
    set_hours_before_drops: Arc<dyn SetHoursBeforeDropsUseCase>,
    set_skip_private: Arc<dyn SetSkipPrivateUseCase>,
    set_skip_refundable: Arc<dyn SetSkipRefundableUseCase>,
    set_restart_games: Arc<dyn SetRestartGamesUseCase>,
    set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
}

impl SettingsViewModel {
    pub fn new(
        get: Arc<dyn GetPreferencesUseCase>,
        set_hours_before_drops: Arc<dyn SetHoursBeforeDropsUseCase>,
        set_skip_private: Arc<dyn SetSkipPrivateUseCase>,
        set_skip_refundable: Arc<dyn SetSkipRefundableUseCase>,
        set_restart_games: Arc<dyn SetRestartGamesUseCase>,
        set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
    ) -> Self {
        Self {
            get,
            set_hours_before_drops,
            set_skip_private,
            set_skip_refundable,
            set_restart_games,
            set_appear_online,
        }
    }

    pub fn hours_before_drops(&self) -> u8 {
        self.get.call().hours_before_drops
    }

    /// An hour more before cards drop, or one fewer, within the choice
    /// there is. Returns the hours now.
    pub fn change_hours_before_drops(&self, more: bool) -> Result<u8, PreferencesError> {
        let hours = self.hours_before_drops();
        let to = if more {
            hours
                .saturating_add(1)
                .min(Preferences::MOST_HOURS_BEFORE_DROPS)
        } else {
            hours.saturating_sub(1)
        };
        self.set_hours_before_drops.call(to)?;
        Ok(to)
    }

    /// An hour more before cards drop, going back to none after the most.
    /// Returns the hours now.
    pub fn cycle_hours_before_drops(&self) -> Result<u8, PreferencesError> {
        let hours = self.hours_before_drops();
        let to = if hours >= Preferences::MOST_HOURS_BEFORE_DROPS {
            0
        } else {
            hours + 1
        };
        self.set_hours_before_drops.call(to)?;
        Ok(to)
    }

    pub fn skip_private(&self) -> bool {
        self.get.call().skip_private
    }

    pub fn toggle_skip_private(&self) -> Result<(), PreferencesError> {
        self.set_skip_private.call(!self.skip_private())
    }

    pub fn skip_refundable(&self) -> bool {
        self.get.call().skip_refundable
    }

    pub fn toggle_skip_refundable(&self) -> Result<(), PreferencesError> {
        self.set_skip_refundable.call(!self.skip_refundable())
    }

    pub fn restart_games(&self) -> bool {
        self.get.call().restart_games
    }

    pub fn toggle_restart_games(&self) -> Result<(), PreferencesError> {
        self.set_restart_games.call(!self.restart_games())
    }

    pub fn appear_online(&self) -> bool {
        self.get.call().appear_online
    }

    pub fn toggle_appear_online(&self) -> Result<(), PreferencesError> {
        self.set_appear_online.call(!self.appear_online())
    }
}

#[cfg(test)]
mod tests {
    use preferences::{
        DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase,
        DefaultSetHoursBeforeDropsUseCase, DefaultSetRestartGamesUseCase,
        DefaultSetSkipPrivateUseCase, DefaultSetSkipRefundableUseCase,
        test_support::FakePreferencesRepository,
    };

    use super::*;

    fn settings() -> SettingsViewModel {
        let repo = Arc::new(FakePreferencesRepository::default());
        SettingsViewModel::new(
            Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            Arc::new(DefaultSetHoursBeforeDropsUseCase::new(repo.clone())),
            Arc::new(DefaultSetSkipPrivateUseCase::new(repo.clone())),
            Arc::new(DefaultSetSkipRefundableUseCase::new(repo.clone())),
            Arc::new(DefaultSetRestartGamesUseCase::new(repo.clone())),
            Arc::new(DefaultSetAppearOnlineUseCase::new(repo)),
        )
    }

    #[test]
    fn the_hours_go_up_and_down_within_the_choice_there_is() {
        let s = settings();
        assert_eq!(s.hours_before_drops(), 3, "3 to start with");

        assert_eq!(s.change_hours_before_drops(false), Ok(2));
        assert_eq!(s.change_hours_before_drops(false), Ok(1));
        assert_eq!(s.change_hours_before_drops(false), Ok(0));
        assert_eq!(
            s.change_hours_before_drops(false),
            Ok(0),
            "none is the least"
        );

        for _ in 0..20 {
            s.change_hours_before_drops(true).unwrap();
        }
        assert_eq!(s.hours_before_drops(), Preferences::MOST_HOURS_BEFORE_DROPS);
    }

    #[test]
    fn cycling_the_hours_goes_back_to_none_after_the_most() {
        let s = settings();
        assert_eq!(s.cycle_hours_before_drops(), Ok(4));
        for _ in 4..Preferences::MOST_HOURS_BEFORE_DROPS {
            s.cycle_hours_before_drops().unwrap();
        }
        assert_eq!(s.hours_before_drops(), Preferences::MOST_HOURS_BEFORE_DROPS);
        assert_eq!(s.cycle_hours_before_drops(), Ok(0));
    }

    #[test]
    fn each_setting_turns_on_and_off_on_its_own() {
        let s = settings();
        assert!(s.skip_private() && s.skip_refundable(), "on to start with");
        assert!(
            !s.restart_games() && !s.appear_online(),
            "off to start with"
        );

        s.toggle_skip_private().unwrap();
        s.toggle_restart_games().unwrap();
        assert!(!s.skip_private() && s.skip_refundable());
        assert!(s.restart_games() && !s.appear_online());

        s.toggle_skip_refundable().unwrap();
        s.toggle_appear_online().unwrap();
        assert!(!s.skip_refundable() && s.appear_online());
    }
}
