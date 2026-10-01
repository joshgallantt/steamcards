//! What each farming event says in the log.

use std::time::Duration;

use card::CardKind;
use farming::{FarmingEvent, Trouble};
use game::HOURS_BEFORE_DROPS;

use crate::counting::{cards, hours, minutes, ordinal};

/// What Steam signing this session off is called, in the log and on the
/// status line.
pub(crate) const SIGNED_OFF: &str = "Steam signed this session off";

/// The log's line for what happened.
pub fn event(event: &FarmingEvent) -> String {
    match event {
        FarmingEvent::FarmingCards { game, cards_left } => {
            format!("Farming {game} — {} to drop", cards(*cards_left))
        }
        FarmingEvent::BuildingHours { lead, games: 1 } => format!(
            "Playing {lead} until it has {HOURS_BEFORE_DROPS} hours, when its cards can start \
             dropping"
        ),
        FarmingEvent::BuildingHours { lead, games } => format!(
            "Playing {games} games together until {lead} has {HOURS_BEFORE_DROPS} hours, when \
             its cards can start dropping"
        ),
        FarmingEvent::HoursBuilt { game } => {
            format!("{game} has {HOURS_BEFORE_DROPS} hours now: its cards can drop.")
        }
        FarmingEvent::NewItems => "Steam says new items arrived".into(),
        FarmingEvent::Looked { game, cards_left } => {
            format!("{game}: {} still to drop", cards(*cards_left))
        }
        FarmingEvent::Dropped { game, count, left } => {
            let what = match count {
                1 => "A card".to_owned(),
                n => format!("{n} cards"),
            };
            match left {
                0 => format!("{what} dropped for {game} — that's all of them"),
                left => format!("{what} dropped for {game} — {left} to go"),
            }
        }
        FarmingEvent::AllDropped { game } => format!("Every card has dropped for {game}."),
        FarmingEvent::Stalled {
            game,
            after,
            for_good,
        } => {
            let what_now = if *for_good {
                "leaving it be: Steam may not drop its cards (a family-shared or free-to-play \
                 game, or one marked private)"
            } else {
                "trying the others first"
            };
            format!("No card from {game} in {} — {what_now}", hours(*after))
        }
        FarmingEvent::MovedOn {
            game,
            outranked: true,
        } => format!("Moving on from {game}: something is ranked higher now"),
        FarmingEvent::MovedOn {
            game,
            outranked: false,
        } => format!("Moving on from {game}: it isn't to be farmed now"),
        FarmingEvent::ChoicesChanged => "Your choices changed: farming in the new order".into(),
        FarmingEvent::Asking { items: 1 } => "Asking Steam which card it was".into(),
        FarmingEvent::Asking { .. } => "Asking Steam which cards they were".into(),
        FarmingEvent::AskFailed { why } => format!("Couldn't ask Steam which card it was: {why}"),
        FarmingEvent::ByCardPage { game } => {
            format!("Steam didn't say which card dropped for {game}: going by its card page")
        }
        FarmingEvent::Identified {
            game,
            card,
            kind,
            copy,
        } => {
            let foil = match kind {
                CardKind::Normal => "",
                CardKind::Foil => " (foil)",
            };
            let copy = match copy {
                Some(1) => String::new(),
                Some(copy) => format!(" (a {} copy)", ordinal(*copy)),
                None => " (which copy isn't known)".into(),
            };
            format!("{card}{foil} dropped for {game}{copy}")
        }
        FarmingEvent::Untold { game } => format!("Couldn't tell which card dropped for {game}"),
        FarmingEvent::CardsUnread { game, why } => {
            format!("Couldn't look at {game}'s cards: {why}")
        }
        FarmingEvent::FoilsUnread { game, why } => {
            format!("Couldn't look at {game}'s foils: {why}")
        }
        FarmingEvent::PlayedElsewhere { game: Some(game) } => {
            format!("{game} is being played on another device: farming waits until it stops.")
        }
        FarmingEvent::PlayedElsewhere { game: None } => {
            "Another device is playing: farming waits until it stops.".into()
        }
        FarmingEvent::TakenOver => {
            "Another device took over playing: farming waits until it stops.".into()
        }
        FarmingEvent::ElsewhereStopped { carry_on_in } => format!(
            "Playing elsewhere stopped: farming carries on in {}.",
            minutes(*carry_on_in)
        ),
        FarmingEvent::WentWrong { trouble, again_in } => match trouble {
            Trouble::BadgesUnread(why) => format!("Couldn't read your badges: {why}"),
            Trouble::Lost(why) => retrying(why, *again_in),
            Trouble::SignedOff => retrying(SIGNED_OFF, *again_in),
            Trouble::Replaced => "Another session signed in with this account's login in \
                                  steamcards' place, so farming stopped rather than knock it off."
                .into(),
        },
    }
}

/// What went wrong, and when it's tried again, if it is.
fn retrying(what: &str, again_in: Option<Duration>) -> String {
    match again_in {
        Some(wait) => format!("{what} — trying again in {}", minutes(wait)),
        None => what.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    fn named(card: &str, kind: CardKind, copy: Option<u32>) -> String {
        event(&FarmingEvent::Identified {
            game: "Heavy Rain".into(),
            card: card.into(),
            kind,
            copy,
        })
    }

    #[test]
    fn playing_says_what_and_until_when() {
        let farming = |cards_left| {
            event(&FarmingEvent::FarmingCards {
                game: "Portal 2".into(),
                cards_left,
            })
        };
        assert_eq!(farming(3), "Farming Portal 2 — 3 cards to drop");
        assert_eq!(farming(1), "Farming Portal 2 — 1 card to drop");
        let building = |games| {
            event(&FarmingEvent::BuildingHours {
                lead: "Game 10".into(),
                games,
            })
        };
        assert_eq!(
            building(1),
            "Playing Game 10 until it has 3 hours, when its cards can start dropping"
        );
        assert_eq!(
            building(3),
            "Playing 3 games together until Game 10 has 3 hours, when its cards can start dropping"
        );
        assert_eq!(
            event(&FarmingEvent::HoursBuilt {
                game: "Game 10".into()
            }),
            "Game 10 has 3 hours now: its cards can drop."
        );
    }

    #[test]
    fn looks_say_how_many_cards_are_left() {
        assert_eq!(
            event(&FarmingEvent::Looked {
                game: "Game 620".into(),
                cards_left: 1
            }),
            "Game 620: 1 card still to drop"
        );
        assert_eq!(
            event(&FarmingEvent::NewItems),
            "Steam says new items arrived"
        );
        assert_eq!(
            event(&FarmingEvent::AllDropped {
                game: "Game 620".into()
            }),
            "Every card has dropped for Game 620."
        );
        assert_eq!(
            event(&FarmingEvent::CardsUnread {
                game: "Hades".into(),
                why: "Steam didn't answer in time".into()
            }),
            "Couldn't look at Hades's cards: Steam didn't answer in time"
        );
    }

    #[test]
    fn drops_say_how_many_and_how_many_are_left() {
        let dropped = |count, left| {
            event(&FarmingEvent::Dropped {
                game: "Heavy Rain".into(),
                count,
                left,
            })
        };
        assert_eq!(dropped(1, 2), "A card dropped for Heavy Rain — 2 to go");
        assert_eq!(dropped(2, 2), "2 cards dropped for Heavy Rain — 2 to go");
        assert_eq!(
            dropped(1, 0),
            "A card dropped for Heavy Rain — that's all of them"
        );
    }

    #[test]
    fn a_named_card_says_which_copy_it_is() {
        assert_eq!(
            named("Madison", CardKind::Normal, Some(2)),
            "Madison dropped for Heavy Rain (a 2nd copy)"
        );
        assert_eq!(
            named("Ethan", CardKind::Normal, Some(1)),
            "Ethan dropped for Heavy Rain"
        );
        assert_eq!(
            named("Scott", CardKind::Foil, Some(3)),
            "Scott (foil) dropped for Heavy Rain (a 3rd copy)"
        );
        assert_eq!(
            named("Norman", CardKind::Normal, None),
            "Norman dropped for Heavy Rain (which copy isn't known)"
        );
    }

    #[test]
    fn telling_cards_apart_says_how_it_went() {
        assert_eq!(
            event(&FarmingEvent::Asking { items: 1 }),
            "Asking Steam which card it was"
        );
        assert_eq!(
            event(&FarmingEvent::Asking { items: 2 }),
            "Asking Steam which cards they were"
        );
        assert_eq!(
            event(&FarmingEvent::AskFailed {
                why: "Steam didn't answer in time".into()
            }),
            "Couldn't ask Steam which card it was: Steam didn't answer in time"
        );
        assert_eq!(
            event(&FarmingEvent::ByCardPage {
                game: "Heavy Rain".into()
            }),
            "Steam didn't say which card dropped for Heavy Rain: going by its card page"
        );
        assert_eq!(
            event(&FarmingEvent::Untold {
                game: "Heavy Rain".into()
            }),
            "Couldn't tell which card dropped for Heavy Rain"
        );
        assert_eq!(
            event(&FarmingEvent::FoilsUnread {
                game: "Hades".into(),
                why: "Steam didn't answer in time".into()
            }),
            "Couldn't look at Hades's foils: Steam didn't answer in time"
        );
    }

    #[test]
    fn moving_on_says_why() {
        let moved_on = |outranked| {
            event(&FarmingEvent::MovedOn {
                game: "Game 1".into(),
                outranked,
            })
        };
        assert_eq!(
            moved_on(true),
            "Moving on from Game 1: something is ranked higher now"
        );
        assert_eq!(
            moved_on(false),
            "Moving on from Game 1: it isn't to be farmed now"
        );
        assert_eq!(
            event(&FarmingEvent::ChoicesChanged),
            "Your choices changed: farming in the new order"
        );
        let stalled = |for_good| {
            event(&FarmingEvent::Stalled {
                game: "Game 1".into(),
                after: 10 * 60 * MINUTE,
                for_good,
            })
        };
        assert_eq!(
            stalled(false),
            "No card from Game 1 in 10 hours — trying the others first"
        );
        assert!(stalled(true).starts_with("No card from Game 1 in 10 hours — leaving it be"));
    }

    #[test]
    fn another_device_is_named_when_its_game_is_known() {
        assert_eq!(
            event(&FarmingEvent::PlayedElsewhere {
                game: Some("Team Fortress 2".into())
            }),
            "Team Fortress 2 is being played on another device: farming waits until it stops."
        );
        assert_eq!(
            event(&FarmingEvent::PlayedElsewhere { game: None }),
            "Another device is playing: farming waits until it stops."
        );
        assert_eq!(
            event(&FarmingEvent::TakenOver),
            "Another device took over playing: farming waits until it stops."
        );
        assert_eq!(
            event(&FarmingEvent::ElsewhereStopped {
                carry_on_in: MINUTE
            }),
            "Playing elsewhere stopped: farming carries on in a minute."
        );
    }

    #[test]
    fn what_went_wrong_says_when_its_tried_again() {
        let went_wrong = |trouble, again_in| event(&FarmingEvent::WentWrong { trouble, again_in });
        assert_eq!(
            went_wrong(
                Trouble::BadgesUnread("steamcommunity.com didn't answer".into()),
                Some(5 * MINUTE)
            ),
            "Couldn't read your badges: steamcommunity.com didn't answer"
        );
        assert_eq!(
            went_wrong(
                Trouble::Lost("the connection to Steam dropped".into()),
                Some(MINUTE)
            ),
            "the connection to Steam dropped — trying again in a minute"
        );
        assert_eq!(
            went_wrong(Trouble::SignedOff, Some(MINUTE)),
            "Steam signed this session off — trying again in a minute"
        );
        assert!(went_wrong(Trouble::Replaced, None).starts_with("Another session signed in"));
    }
}
