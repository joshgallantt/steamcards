//! Someone arranging what gets farmed first, through the preferences' use
//! cases over a fake repository: the rules, with nothing else in the way.

use std::sync::Arc;

use game::AppId;
use preferences::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetAutoUpdateUseCase,
    DefaultSetGameTierUseCase, DefaultSetOnlyPriorityUseCase, DefaultSetRestartGamesUseCase,
    GetPreferencesUseCase, Preferences, SetAppearOnlineUseCase, SetAutoUpdateUseCase,
    SetGameTierUseCase, SetOnlyPriorityUseCase, SetRestartGamesUseCase, Tier,
    test_support::FakePreferencesRepository,
};

/// Someone arranging what gets farmed first.
pub(crate) struct Player {
    get: Arc<dyn GetPreferencesUseCase>,
    pub(crate) tier: Arc<dyn SetGameTierUseCase>,
    pub(crate) only: Arc<dyn SetOnlyPriorityUseCase>,
    pub(crate) online: Arc<dyn SetAppearOnlineUseCase>,
    pub(crate) restart: Arc<dyn SetRestartGamesUseCase>,
    pub(crate) auto_update: Arc<dyn SetAutoUpdateUseCase>,
}

impl Player {
    pub(crate) fn new() -> Self {
        Self::on(FakePreferencesRepository::default())
    }

    pub(crate) fn whose_disk_is_full() -> Self {
        Self::on(FakePreferencesRepository::default().failing())
    }

    fn on(repo: FakePreferencesRepository) -> Self {
        let repo = Arc::new(repo);
        Self {
            get: Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            tier: Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            only: Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            online: Arc::new(DefaultSetAppearOnlineUseCase::new(repo.clone())),
            restart: Arc::new(DefaultSetRestartGamesUseCase::new(repo.clone())),
            auto_update: Arc::new(DefaultSetAutoUpdateUseCase::new(repo)),
        }
    }

    pub(crate) fn prefs(&self) -> Preferences {
        self.get.call()
    }

    /// The games farmed first, in order.
    pub(crate) fn priorities(&self) -> Vec<u32> {
        self.prefs().priority_games.iter().map(|g| g.0).collect()
    }

    /// The games never farmed.
    pub(crate) fn skipped(&self) -> Vec<u32> {
        self.prefs().skipped_games.iter().map(|g| g.0).collect()
    }

    pub(crate) fn ranks(&self, app_id: u32, rank: usize) {
        self.tier.call(AppId(app_id), Tier::Priority(rank)).unwrap();
    }

    pub(crate) fn sets(&self, app_id: u32, tier: Tier) {
        self.tier.call(AppId(app_id), tier).unwrap();
    }
}
