use std::sync::Arc;

use crate::games::GameRow;

use game::{AppId, Game};
use preferences::{
    GetPreferencesUseCase, PreferencesError, SetAppearOnlineUseCase, SetGameTierUseCase,
    SetOnlyPriorityUseCase, Tier,
};

/// Choosing which games are farmed first, and how farming shows to friends.
pub struct GamesViewModel {
    get: Arc<dyn GetPreferencesUseCase>,
    set_tier: Arc<dyn SetGameTierUseCase>,
    set_only_priority: Arc<dyn SetOnlyPriorityUseCase>,
    set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
}

impl GamesViewModel {
    pub fn new(
        get: Arc<dyn GetPreferencesUseCase>,
        set_tier: Arc<dyn SetGameTierUseCase>,
        set_only_priority: Arc<dyn SetOnlyPriorityUseCase>,
        set_appear_online: Arc<dyn SetAppearOnlineUseCase>,
    ) -> Self {
        Self {
            get,
            set_tier,
            set_only_priority,
            set_appear_online,
        }
    }

    pub fn only_priority(&self) -> bool {
        self.get.call().only_priority
    }

    pub fn toggle_only_priority(&self) -> Result<(), PreferencesError> {
        self.set_only_priority.call(!self.only_priority())
    }

    pub fn appear_online(&self) -> bool {
        self.get.call().appear_online
    }

    pub fn toggle_appear_online(&self) -> Result<(), PreferencesError> {
        self.set_appear_online.call(!self.appear_online())
    }

    pub fn set_tier(&self, app_id: AppId, tier: Tier) -> Result<(), PreferencesError> {
        self.set_tier.call(app_id, tier)
    }

    /// Picks a game — it goes to the end of the priority list — or, if it's
    /// picked already, unpicks it. Returns its rank now.
    pub fn toggle(&self, app_id: AppId) -> Result<Option<usize>, PreferencesError> {
        let prefs = self.get.call();
        let tier = match prefs.rank(app_id) {
            Some(_) => Tier::Indifferent,
            None => Tier::Priority(prefs.priority_games.len() + 1),
        };
        self.set_tier.call(app_id, tier)?;
        Ok(self.get.call().rank(app_id).map(|i| i + 1))
    }

    /// Priority games in their order, then every other game with drops left
    /// in `games`, so the user can see where each stands.
    pub fn rows(&self, games: &[Game]) -> Vec<GameRow> {
        let prefs = self.get.call();
        let mut out: Vec<GameRow> = prefs
            .priority_games
            .iter()
            .map(|&id| row(id, games, prefs.rank(id)))
            .collect();
        out.extend(
            games
                .iter()
                .filter(|g| g.has_drops_left() && prefs.rank(g.app_id).is_none())
                .map(|g| row(g.app_id, games, None)),
        );
        out
    }

    /// `games` with drops left, in their own order whatever is picked, so
    /// picking one doesn't move it; picked games they don't have come last.
    pub fn picks(&self, games: &[Game]) -> Vec<GameRow> {
        let prefs = self.get.call();
        let mut out: Vec<GameRow> = games
            .iter()
            .filter(|g| g.has_drops_left())
            .map(|g| row(g.app_id, games, prefs.rank(g.app_id)))
            .collect();
        out.extend(
            prefs
                .priority_games
                .iter()
                .filter(|&&id| !out.iter().any(|r| r.app_id == id))
                .map(|&id| row(id, games, prefs.rank(id)))
                .collect::<Vec<_>>(),
        );
        out
    }
}

fn row(app_id: AppId, games: &[Game], rank: Option<usize>) -> GameRow {
    let game = games.iter().find(|g| g.app_id == app_id);
    GameRow {
        app_id,
        name: game.map_or_else(|| format!("App {app_id}"), |g| g.name.clone()),
        rank: rank.map(|i| i + 1),
        hours: game.map_or(0.0, |g| g.hours),
        drops: game.map(|g| g.drops).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use game::{AppId, test_support::game};
    use preferences::{
        DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetGameTierUseCase,
        DefaultSetOnlyPriorityUseCase, Preferences, test_support::FakePreferencesRepository,
    };

    use super::*;

    fn games(priority: &[u32]) -> GamesViewModel {
        let repo = Arc::new(FakePreferencesRepository::new(Preferences {
            priority_games: priority.iter().copied().map(AppId).collect(),
            ..Default::default()
        }));
        GamesViewModel::new(
            Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            Arc::new(DefaultSetAppearOnlineUseCase::new(repo)),
        )
    }

    fn library() -> Vec<Game> {
        vec![
            game(10, 5.0, 1, 3),
            game(20, 1.0, 0, 2),
            game(30, 9.0, 4, 0),
        ]
    }

    fn ids(rows: &[GameRow]) -> Vec<(u32, Option<usize>)> {
        rows.iter().map(|r| (r.app_id.0, r.rank)).collect()
    }

    #[test]
    fn priority_games_first_then_the_rest_with_drops_left() {
        let g = games(&[20, 99]);
        assert_eq!(
            ids(&g.rows(&library())),
            [(20, Some(1)), (99, Some(2)), (10, None)],
            "finished games aren't offered; a priority the library lacks still shows"
        );
        assert_eq!(g.rows(&library())[1].name, "App 99");
    }

    #[test]
    fn picks_keep_the_library_order_whatever_is_picked() {
        let g = games(&[20]);
        assert_eq!(ids(&g.picks(&library())), [(10, None), (20, Some(1))]);
    }

    #[test]
    fn toggling_picks_a_game_last_then_unpicks_it() {
        let g = games(&[20]);
        assert_eq!(g.toggle(AppId(10)), Ok(Some(2)));
        assert_eq!(g.toggle(AppId(20)), Ok(None));
        assert_eq!(
            ids(&g.rows(&library())),
            [(10, Some(1)), (20, None)],
            "unpicked, it's still there to pick"
        );
    }

    #[test]
    fn appearing_online_and_only_priority_toggle() {
        let g = games(&[]);
        assert!(!g.appear_online(), "offline by default");
        g.toggle_appear_online().unwrap();
        assert!(g.appear_online());
        g.toggle_only_priority().unwrap();
        assert!(g.only_priority());
    }
}
