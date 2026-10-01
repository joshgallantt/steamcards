//! Unit tier: the rules for what gets farmed first, the preferences' use
//! cases over a fake repository. Every verb is the user's. The acceptance
//! tier, through the preferences component and a real config file, is in
//! preferences-di.

mod support;

use game::AppId;
use preferences::{PreferencesError, Tier};
use support::Player;

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
