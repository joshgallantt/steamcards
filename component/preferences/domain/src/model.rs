use std::fmt;

/// Games are named by their Steam app ID throughout: a name can change, and
/// two games can share one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preferences {
    /// Games farmed first, in this order; index 0 is the user's #1.
    pub priority_games: Vec<u32>,
    /// Games never farmed.
    pub skipped_games: Vec<u32>,
    /// Farm the priority games only.
    pub only_priority: bool,
    /// Show as online to friends while farming, games and all. Off (the
    /// default), the account appears offline, and Steam counts the games
    /// just the same.
    pub appear_online: bool,
}

impl Preferences {
    /// A game's place among the priority games, from 0.
    pub fn rank(&self, app_id: u32) -> Option<usize> {
        self.priority_games.iter().position(|&g| g == app_id)
    }

    pub fn is_skipped(&self, app_id: u32) -> bool {
        self.skipped_games.contains(&app_id)
    }

    pub fn tier(&self, app_id: u32) -> Tier {
        if self.is_skipped(app_id) {
            Tier::Skip
        } else {
            self.rank(app_id)
                .map_or(Tier::Indifferent, |r| Tier::Priority(r + 1))
        }
    }

    /// Whether a game is farmed at all: not skipped, and a priority when
    /// "only priority" is on.
    pub fn wants(&self, app_id: u32) -> bool {
        match self.tier(app_id) {
            Tier::Priority(_) => true,
            Tier::Indifferent => !self.only_priority,
            Tier::Skip => false,
        }
    }
}

/// How much the user wants a game farmed: priority -> indifferent -> skip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// 1-based position among the priority games.
    Priority(usize),
    /// The default: farmed after the priorities, in the farmer's own order.
    Indifferent,
    /// Never farmed.
    Skip,
}

/// Why a change to the preferences didn't happen, in the user's terms. A disk
/// that won't write and a file that can't be renamed are one fact to them: the
/// change didn't stick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesError {
    Unavailable,
}

impl fmt::Display for PreferencesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreferencesError::Unavailable => write!(f, "preferences couldn't be saved"),
        }
    }
}

impl std::error::Error for PreferencesError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_reflects_priority_and_skip_lists() {
        let prefs = Preferences {
            priority_games: vec![620, 440],
            skipped_games: vec![730],
            ..Default::default()
        };
        assert_eq!(prefs.tier(440), Tier::Priority(2));
        assert_eq!(prefs.tier(730), Tier::Skip);
        assert_eq!(prefs.tier(220), Tier::Indifferent);
    }

    #[test]
    fn only_priority_leaves_the_rest_out() {
        let mut prefs = Preferences {
            priority_games: vec![620],
            skipped_games: vec![730],
            ..Default::default()
        };
        assert!(prefs.wants(620) && prefs.wants(220) && !prefs.wants(730));
        prefs.only_priority = true;
        assert!(prefs.wants(620) && !prefs.wants(220));
    }
}
