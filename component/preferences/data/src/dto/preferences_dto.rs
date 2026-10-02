use game::{AppId, HOURS_BEFORE_DROPS};
use preferences::Preferences;
use serde::{Deserialize, Serialize};

/// The preferences' fields in the config file, each at the top of it: games
/// by app ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PreferencesDto {
    #[serde(default)]
    priority_games: Vec<u32>,
    #[serde(default)]
    skipped_games: Vec<u32>,
    #[serde(default)]
    only_priority: bool,
    #[serde(default)]
    appear_online: bool,
    #[serde(default)]
    restart_games: bool,
    /// On unless turned off, so a file from before it was a choice keeps
    /// steamcards up to date.
    #[serde(default = "on")]
    auto_update: bool,
    /// From before it was a choice: 3.
    #[serde(default = "hours_before_drops")]
    hours_before_drops: u8,
    /// On unless turned off, as `auto_update` is.
    #[serde(default = "on")]
    skip_private: bool,
    #[serde(default = "on")]
    skip_refundable: bool,
}

fn on() -> bool {
    true
}

fn hours_before_drops() -> u8 {
    HOURS_BEFORE_DROPS
}

impl Default for PreferencesDto {
    fn default() -> Self {
        Self::new(&Preferences::default())
    }
}

impl PreferencesDto {
    pub(crate) fn new(p: &Preferences) -> Self {
        Self {
            priority_games: p.priority_games.iter().map(|g| g.0).collect(),
            skipped_games: p.skipped_games.iter().map(|g| g.0).collect(),
            only_priority: p.only_priority,
            appear_online: p.appear_online,
            restart_games: p.restart_games,
            auto_update: p.auto_update,
            hours_before_drops: p.hours_before_drops,
            skip_private: p.skip_private,
            skip_refundable: p.skip_refundable,
        }
    }

    pub(crate) fn into_domain(self) -> Preferences {
        Preferences {
            priority_games: self.priority_games.into_iter().map(AppId).collect(),
            skipped_games: self.skipped_games.into_iter().map(AppId).collect(),
            only_priority: self.only_priority,
            appear_online: self.appear_online,
            restart_games: self.restart_games,
            auto_update: self.auto_update,
            // A file edited by hand can say more than there's a choice of.
            hours_before_drops: self
                .hours_before_drops
                .min(Preferences::MOST_HOURS_BEFORE_DROPS),
            skip_private: self.skip_private,
            skip_refundable: self.skip_refundable,
        }
    }
}
