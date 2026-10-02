//! Acceptance tier: choosing what gets farmed first, as the user meets it,
//! through the preferences component as steamcards wires it, kept in a real
//! config file.

mod support;

use preferences::{PreferencesError, Tier};
use support::Player;

#[test]
fn a_game_moves_between_tiers() {
    let player = Player::new("tiers");

    player.ranks(620, 1);
    player.ranks(440, 9); // past the end: goes last
    assert_eq!(player.priorities(), [620, 440]);

    player.ranks(220, 1); // bumps the others down
    assert_eq!(player.priorities(), [220, 620, 440]);

    player.sets(620, Tier::Skip).unwrap(); // leaves priority
    assert_eq!(player.priorities(), [220, 440]);
    assert_eq!(player.skipped(), [620]);
}

#[test]
fn what_was_chosen_is_there_when_steamcards_starts_again() {
    let player = Player::new("kept");
    player.ranks(620, 1);
    player.sets(730, Tier::Skip).unwrap();
    player.appears_online(true).unwrap();
    player.farms_only_priority(true).unwrap();
    player.sets_hours_before_drops(0).unwrap();
    player.skips_private_games(false).unwrap();
    player.skips_refundable_games(false).unwrap();

    let player = player.comes_back();

    assert_eq!(player.priorities(), [620]);
    assert_eq!(player.skipped(), [730]);
    let prefs = player.prefs();
    assert!(prefs.appear_online && prefs.only_priority);
    assert_eq!(prefs.hours_before_drops, 0);
    assert!(!prefs.skip_private && !prefs.skip_refundable);
}

#[test]
fn appearing_offline_is_the_default() {
    let player = Player::new("offline");
    assert!(!player.prefs().appear_online);
}

#[test]
fn a_change_that_did_not_stick_says_so() {
    let player = Player::new("full-disk");
    player.runs_out_of_disk();

    assert_eq!(
        player.sets(620, Tier::Skip),
        Err(PreferencesError::Unavailable)
    );
    assert_eq!(
        player.appears_online(true),
        Err(PreferencesError::Unavailable)
    );
    assert!(player.skipped().is_empty(), "nothing changed");
}
