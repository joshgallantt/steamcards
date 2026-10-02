//! Unit tier: the rules for what gets farmed first, the preferences' use
//! cases over a fake repository. Every verb is the user's. The acceptance
//! tier, through the preferences component and a real config file, is in
//! preferences-di.

mod support;

use game::AppId;
use preferences::{Preferences, PreferencesError, Tier};
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
fn restarting_games_is_off_until_asked_for() {
    let player = Player::new();
    assert!(!player.prefs().restart_games);
    player.restart.call(true).unwrap();
    assert!(player.prefs().restart_games);
    player.restart.call(false).unwrap();
    assert!(!player.prefs().restart_games);
}

#[test]
fn automatic_updates_are_on_until_turned_off() {
    let player = Player::new();
    assert!(player.prefs().auto_update);
    player.auto_update.call(false).unwrap();
    assert!(!player.prefs().auto_update);
}

#[test]
fn games_need_three_hours_before_their_cards_drop_until_told_otherwise() {
    let player = Player::new();
    assert_eq!(player.prefs().hours_before_drops, 3);

    player.hours.call(0).unwrap();
    assert_eq!(
        player.prefs().hours_before_drops,
        0,
        "every game on its own"
    );
    player.hours.call(2).unwrap();
    assert_eq!(player.prefs().hours_before_drops, 2);
    player.hours.call(200).unwrap();
    assert_eq!(
        player.prefs().hours_before_drops,
        Preferences::MOST_HOURS_BEFORE_DROPS,
        "no more than there's a choice of"
    );
}

#[test]
fn private_games_and_games_steam_would_refund_are_left_out_until_asked_for() {
    let player = Player::new();
    assert!(player.prefs().skip_private && player.prefs().skip_refundable);

    player.skip_private.call(false).unwrap();
    assert!(!player.prefs().skip_private);
    assert!(player.prefs().skip_refundable, "each on its own");
    player.skip_refundable.call(false).unwrap();
    assert!(!player.prefs().skip_refundable);
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
    assert_eq!(player.hours.call(0), Err(PreferencesError::Unavailable));
    assert_eq!(
        player.skip_private.call(false),
        Err(PreferencesError::Unavailable)
    );
    assert!(player.skipped().is_empty(), "nothing changed");
    assert_eq!(player.prefs().hours_before_drops, 3);
}
