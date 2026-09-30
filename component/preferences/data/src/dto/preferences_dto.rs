use game::AppId;
use preferences::Preferences;
use serde::{Deserialize, Serialize};

/// The preferences' fields in the config file, each at the top of it: games
/// by app ID.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PreferencesDto {
    #[serde(default)]
    priority_games: Vec<u32>,
    #[serde(default)]
    skipped_games: Vec<u32>,
    #[serde(default)]
    only_priority: bool,
    #[serde(default)]
    appear_online: bool,
}

impl PreferencesDto {
    pub(crate) fn new(p: &Preferences) -> Self {
        Self {
            priority_games: p.priority_games.iter().map(|g| g.0).collect(),
            skipped_games: p.skipped_games.iter().map(|g| g.0).collect(),
            only_priority: p.only_priority,
            appear_online: p.appear_online,
        }
    }

    pub(crate) fn into_domain(self) -> Preferences {
        Preferences {
            priority_games: self.priority_games.into_iter().map(AppId).collect(),
            skipped_games: self.skipped_games.into_iter().map(AppId).collect(),
            only_priority: self.only_priority,
            appear_online: self.appear_online,
        }
    }
}
