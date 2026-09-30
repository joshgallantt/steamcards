//! Acceptance tier: the rules as the user meets them. Every verb is theirs.

use std::sync::Arc;

use game::AppId;
use preferences::{
    DefaultGetPreferencesUseCase, DefaultSetAppearOnlineUseCase, DefaultSetGameTierUseCase,
    DefaultSetOnlyPriorityUseCase, GetPreferencesUseCase, Preferences, PreferencesError,
    SetAppearOnlineUseCase, SetGameTierUseCase, SetOnlyPriorityUseCase, Tier,
    test_support::InMemoryPreferencesRepository,
};

/// Someone arranging what gets farmed first.
struct Player {
    get: Arc<dyn GetPreferencesUseCase>,
    tier: Arc<dyn SetGameTierUseCase>,
    only: Arc<dyn SetOnlyPriorityUseCase>,
    online: Arc<dyn SetAppearOnlineUseCase>,
}

impl Player {
    fn new() -> Self {
        Self::on(InMemoryPreferencesRepository::default())
    }

    fn whose_disk_is_full() -> Self {
        Self::on(InMemoryPreferencesRepository::default().failing())
    }

    fn on(repo: InMemoryPreferencesRepository) -> Self {
        let repo = Arc::new(repo);
        Self {
            get: Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            tier: Arc::new(DefaultSetGameTierUseCase::new(repo.clone())),
            only: Arc::new(DefaultSetOnlyPriorityUseCase::new(repo.clone())),
            online: Arc::new(DefaultSetAppearOnlineUseCase::new(repo)),
        }
    }

    fn prefs(&self) -> Preferences {
        self.get.call()
    }

    /// The games farmed first, in order.
    fn priorities(&self) -> Vec<u32> {
        self.prefs().priority_games.iter().map(|g| g.0).collect()
    }

    /// The games never farmed.
    fn skipped(&self) -> Vec<u32> {
        self.prefs().skipped_games.iter().map(|g| g.0).collect()
    }

    fn ranks(&self, app_id: u32, rank: usize) {
        self.tier.call(AppId(app_id), Tier::Priority(rank)).unwrap();
    }

    fn sets(&self, app_id: u32, tier: Tier) {
        self.tier.call(AppId(app_id), tier).unwrap();
    }
}

#[test]
fn a_game_moves_between_tiers() {
    let player = Player::new();

    player.ranks(620, 1);
    player.ranks(440, 9); // past the end: goes last
    assert_eq!(player.priorities(), [620, 440]);

    player.ranks(220, 1); // bumps the others down
    assert_eq!(player.priorities(), [220, 620, 440]);

    player.ranks(440, 2); // moves, no duplicate
    assert_eq!(player.priorities(), [220, 440, 620]);

    player.sets(220, Tier::Indifferent);
    assert_eq!(player.priorities(), [440, 620]);

    player.sets(620, Tier::Skip); // leaves priority
    assert_eq!(player.priorities(), [440]);
    assert_eq!(player.skipped(), [620]);

    player.ranks(620, 1); // un-skips
    assert_eq!(player.priorities(), [620, 440]);
    assert!(player.skipped().is_empty());
}

#[test]
fn appearing_offline_is_the_default() {
    let player = Player::new();
    assert!(!player.prefs().appear_online);
    player.online.call(true).unwrap();
    assert!(player.prefs().appear_online);
}

#[test]
fn only_priority_is_kept() {
    let player = Player::new();
    player.only.call(true).unwrap();
    assert!(player.prefs().only_priority);
}

#[test]
fn a_change_that_did_not_stick_says_so() {
    let player = Player::whose_disk_is_full();
    assert_eq!(
        player.tier.call(AppId(620), Tier::Skip),
        Err(PreferencesError::Unavailable)
    );
    assert_eq!(player.online.call(true), Err(PreferencesError::Unavailable));
    assert!(player.skipped().is_empty(), "nothing changed");
}
