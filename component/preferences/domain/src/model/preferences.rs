use steam_library::AppId;

use crate::Tier;

/// Games are named by their Steam app ID throughout: a name can change, and
/// two games can share one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preferences {
    /// Games farmed first, in this order; index 0 is the user's #1.
    pub priority_games: Vec<AppId>,
    /// Games never farmed.
    pub skipped_games: Vec<AppId>,
    /// Farm the priority games only.
    pub only_priority: bool,
    /// Show as online to friends while farming, games and all. Off (the
    /// default), the account appears offline, and Steam counts the games
    /// just the same.
    pub appear_online: bool,
}

impl Preferences {
    /// A game's place among the priority games, from 0.
    pub fn rank(&self, app_id: AppId) -> Option<usize> {
        self.priority_games.iter().position(|&g| g == app_id)
    }

    pub fn is_skipped(&self, app_id: AppId) -> bool {
        self.skipped_games.contains(&app_id)
    }

    pub fn tier(&self, app_id: AppId) -> Tier {
        if self.is_skipped(app_id) {
            Tier::Skip
        } else {
            self.rank(app_id)
                .map_or(Tier::Indifferent, |r| Tier::Priority(r + 1))
        }
    }

    /// Whether a game is farmed at all: not skipped, and a priority when
    /// "only priority" is on.
    pub fn wants(&self, app_id: AppId) -> bool {
        match self.tier(app_id) {
            Tier::Priority(_) => true,
            Tier::Indifferent => !self.only_priority,
            Tier::Skip => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_reflects_priority_and_skip_lists() {
        let prefs = Preferences {
            priority_games: vec![AppId(620), AppId(440)],
            skipped_games: vec![AppId(730)],
            ..Default::default()
        };
        assert_eq!(prefs.tier(AppId(440)), Tier::Priority(2));
        assert_eq!(prefs.tier(AppId(730)), Tier::Skip);
        assert_eq!(prefs.tier(AppId(220)), Tier::Indifferent);
    }

    #[test]
    fn only_priority_leaves_the_rest_out() {
        let mut prefs = Preferences {
            priority_games: vec![AppId(620)],
            skipped_games: vec![AppId(730)],
            ..Default::default()
        };
        assert!(prefs.wants(AppId(620)) && prefs.wants(AppId(220)) && !prefs.wants(AppId(730)));
        prefs.only_priority = true;
        assert!(prefs.wants(AppId(620)) && !prefs.wants(AppId(220)));
    }
}
