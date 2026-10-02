use game::{AppId, HOURS_BEFORE_DROPS};

use crate::Tier;

/// Games are named by their Steam app ID throughout: a name can change, and
/// two games can share one.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// Stop the game being farmed every 5 minutes, and play it again a moment
    /// later, to shake drops loose, as Steam Game Idler does. Off unless the
    /// user turns it on.
    pub restart_games: bool,
    /// Keep steamcards up to date: look for a new release once a day, and
    /// put it in place, or say how to get it. On unless the user turns it
    /// off.
    pub auto_update: bool,
    /// Hours a game needs on record before its cards drop on this account:
    /// games short of them play together to build them, and a game is
    /// farmed on its own once it has them. 3 unless the user says otherwise;
    /// 0 farms every game on its own, for an account Steam doesn't hold
    /// back. At most [`Self::MOST_HOURS_BEFORE_DROPS`].
    pub hours_before_drops: u8,
    /// Leave out games marked private: Steam drops no cards for them. On
    /// unless the user turns it off.
    pub skip_private: bool,
    /// Leave out games Steam would still refund, bought in the last 14 days
    /// and played under 2 hours, so farming them doesn't cost the refund. On
    /// unless the user turns it off.
    pub skip_refundable: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            priority_games: Vec::new(),
            skipped_games: Vec::new(),
            only_priority: false,
            appear_online: false,
            restart_games: false,
            auto_update: true,
            hours_before_drops: HOURS_BEFORE_DROPS,
            skip_private: true,
            skip_refundable: true,
        }
    }
}

impl Preferences {
    /// The most hours before cards drop there's a choice of.
    pub const MOST_HOURS_BEFORE_DROPS: u8 = 10;

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
