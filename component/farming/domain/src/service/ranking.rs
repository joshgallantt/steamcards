//! What to play, and how. Pure: the library, the preferences, what's been
//! set aside and the time go in; an order and a plan come out.

use chrono::{DateTime, Utc};
use game::{AppId, Game, MOST_PLAYED_AT_ONCE, SteamLibrary};
use preferences::Preferences;
use session::SetAside;

use crate::{
    LeftOut, NothingToFarm,
    model::{
        Plan,
        rules::{GIVE_UP_TIMES, SALE_EVENTS},
    },
};

/// Why `game` isn't farmed at `now`, if it isn't: the user skipped it, or
/// "only priority" leaves it out; it's a sale event's badge, which playing
/// never drops; or, as the user chose, it's private, or Steam would still
/// refund it. A game with no drops left isn't left out: it's done.
///
/// Public so a screen can say why a game waits.
pub fn left_out(game: &Game, prefs: &Preferences, now: DateTime<Utc>) -> Option<LeftOut> {
    if prefs.is_skipped(game.app_id) {
        Some(LeftOut::Skipped)
    } else if !prefs.wants(game.app_id) {
        Some(LeftOut::NotPriority)
    } else if SALE_EVENTS.contains(&game.app_id) {
        Some(LeftOut::SaleEvent)
    } else if prefs.skip_private && game.private {
        Some(LeftOut::Private)
    } else if prefs.skip_refundable && game.is_refundable(now) {
        game.refund_ends()
            .map(|until| LeftOut::Refundable { until })
    } else {
        None
    }
}

/// The games worth farming at `now`, by app ID, in the order they're
/// farmed:
///
/// 1. the user's priority games, in their order;
/// 2. games whose cards can drop now, fewest drops left first, so games
///    finish sooner (as Steam Game Idler orders them);
/// 3. games still building hours, most hours first: the closest to dropping;
/// 4. games set aside after a long while without a drop, last.
///
/// Left out: finished games; games [`left_out`] for the user's choices or
/// what Steam says of them; and games set aside too often this run.
///
/// Public so a screen can size up the job before farming starts, in the
/// order the farmer will take it.
pub fn farm_order(
    library: &SteamLibrary,
    prefs: &Preferences,
    set_aside: &[SetAside],
    now: DateTime<Utc>,
) -> Vec<AppId> {
    let aside = |g: &Game| {
        set_aside
            .iter()
            .find(|s| s.app_id == g.app_id)
            .map_or(0, |s| s.times)
    };
    let before_drops = prefs.hours_before_drops;
    let mut games: Vec<&Game> = library
        .with_drops_left()
        .filter(|g| left_out(g, prefs, now).is_none())
        .filter(|g| aside(g) < GIVE_UP_TIMES)
        .collect();
    games.sort_by(|a, b| {
        let rank = |g: &Game| prefs.rank(g.app_id).unwrap_or(usize::MAX);
        let (a_drops, b_drops) = (a.can_drop(before_drops), b.can_drop(before_drops));
        aside(a)
            .cmp(&aside(b))
            .then(rank(a).cmp(&rank(b)))
            .then(b_drops.cmp(&a_drops))
            .then_with(|| {
                if a_drops && b_drops {
                    a.drops.remaining.cmp(&b.drops.remaining)
                } else {
                    b.hours.total_cmp(&a.hours)
                }
            })
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    games.into_iter().map(|g| g.app_id).collect()
}

/// What to play next: the first game in the order on its own when its cards
/// can drop; otherwise it and the other games still building hours, in
/// order, as many as Steam counts at once. Cards drop for one game at a
/// time: playing several together only builds their hours (ASF, and Steam
/// Game Idler since 6.2). On an account Steam doesn't hold back, every
/// game's cards can drop, so every game is farmed on its own.
pub(crate) fn plan(
    library: &SteamLibrary,
    prefs: &Preferences,
    set_aside: &[SetAside],
    now: DateTime<Utc>,
) -> Plan {
    let before_drops = prefs.hours_before_drops;
    let order = farm_order(library, prefs, set_aside, now);
    let Some(first) = order.first().and_then(|&id| library.game(id)) else {
        return Plan::Nothing;
    };
    if first.can_drop(before_drops) {
        return Plan::Cards(first.app_id);
    }
    Plan::Hours(
        order
            .iter()
            .copied()
            .filter(|&id| library.game(id).is_some_and(|g| !g.can_drop(before_drops)))
            .take(MOST_PLAYED_AT_ONCE)
            .collect(),
    )
}

/// Why nothing is being farmed at `now`, in the user's words.
pub(crate) fn why_nothing(
    library: &SteamLibrary,
    prefs: &Preferences,
    now: DateTime<Utc>,
) -> NothingToFarm {
    if library.is_empty() {
        return NothingToFarm::NoGames;
    }
    if library.drops_left() == 0 {
        return NothingToFarm::AllDropped;
    }
    let why: Vec<Option<LeftOut>> = library
        .with_drops_left()
        .map(|g| left_out(g, prefs, now))
        .collect();
    // Some are wanted, and still nothing's farmed: set aside too often.
    if why.contains(&None) {
        return NothingToFarm::NotDropping;
    }
    let private = why.contains(&Some(LeftOut::Private));
    let refundable_until = why
        .iter()
        .filter_map(|w| match w {
            Some(LeftOut::Refundable { until }) => Some(*until),
            _ => None,
        })
        .min();
    let chosen = why
        .iter()
        .all(|w| matches!(w, Some(LeftOut::Skipped | LeftOut::NotPriority)));
    if private || refundable_until.is_some() {
        NothingToFarm::HeldBack {
            private,
            refundable_until,
        }
    } else if !chosen {
        NothingToFarm::NotDropping
    } else if prefs.only_priority {
        NothingToFarm::PrioritiesDone
    } else {
        NothingToFarm::AllSkipped
    }
}

/// When the first game Steam would still refund stops being refundable, of
/// those left out for it at `now`: farming looks again then.
pub(crate) fn first_refund_ends(
    library: &SteamLibrary,
    prefs: &Preferences,
    now: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    library
        .with_drops_left()
        .filter_map(|g| match left_out(g, prefs, now) {
            Some(LeftOut::Refundable { until }) => Some(until),
            _ => None,
        })
        .min()
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, TimeDelta};
    use game::CardDrops;

    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::from_timestamp(1_790_553_600, 0).unwrap()
    }

    fn game(app_id: u32, hours: f64, remaining: u32) -> Game {
        Game {
            app_id: AppId(app_id),
            name: format!("Game {app_id}"),
            hours,
            drops: CardDrops {
                received: 0,
                remaining,
            },
            badge_level: 0,
            private: false,
            bought_at: None,
        }
    }

    fn library(games: Vec<Game>) -> SteamLibrary {
        SteamLibrary::new(games)
    }

    fn ids(app_ids: &[u32]) -> Vec<AppId> {
        app_ids.iter().copied().map(AppId).collect()
    }

    fn priorities(app_ids: &[u32]) -> Preferences {
        Preferences {
            priority_games: ids(app_ids),
            ..Default::default()
        }
    }

    fn aside(app_id: u32, times: u8) -> SetAside {
        SetAside {
            app_id: AppId(app_id),
            times,
            since: DateTime::default(),
        }
    }

    #[test]
    fn games_that_can_drop_go_first_fewest_drops_left_first() {
        let lib = library(vec![
            game(1, 5.0, 4),
            game(2, 0.5, 3),
            game(3, 8.0, 1),
            game(4, 2.9, 2),
        ]);
        let order = farm_order(&lib, &Preferences::default(), &[], now());
        assert_eq!(order, ids(&[3, 1, 4, 2]), "then the most hours first");
    }

    #[test]
    fn the_users_priorities_come_before_everything() {
        let lib = library(vec![game(1, 5.0, 4), game(2, 0.5, 3), game(3, 8.0, 1)]);
        let order = farm_order(&lib, &priorities(&[2, 1]), &[], now());
        assert_eq!(order, ids(&[2, 1, 3]));
    }

    #[test]
    fn unwanted_finished_and_sale_event_games_are_left_out() {
        let lib = library(vec![
            game(1, 5.0, 4),
            game(2, 5.0, 0),
            game(3, 5.0, 2),
            game(2_861_720, 0.0, 3),
            game(5, 5.0, 1),
        ]);
        let prefs = Preferences {
            skipped_games: ids(&[3]),
            ..Default::default()
        };
        assert_eq!(farm_order(&lib, &prefs, &[], now()), ids(&[5, 1]));

        let only = Preferences {
            priority_games: ids(&[1]),
            only_priority: true,
            ..Default::default()
        };
        assert_eq!(farm_order(&lib, &only, &[], now()), ids(&[1]));
    }

    #[test]
    fn a_game_set_aside_waits_behind_the_rest_then_is_left_alone() {
        let lib = library(vec![game(1, 5.0, 1), game(2, 5.0, 2)]);
        let once = [aside(1, 1)];
        assert_eq!(
            farm_order(&lib, &Preferences::default(), &once, now()),
            ids(&[2, 1])
        );
        let twice = [aside(1, 2)];
        assert_eq!(
            farm_order(&lib, &Preferences::default(), &twice, now()),
            ids(&[2])
        );
    }

    #[test]
    fn a_game_that_can_drop_is_played_alone() {
        let lib = library(vec![game(1, 5.0, 2), game(2, 1.0, 3)]);
        assert_eq!(
            plan(&lib, &Preferences::default(), &[], now()),
            Plan::Cards(AppId(1))
        );
    }

    #[test]
    fn games_building_hours_are_played_together_up_to_32() {
        let games: Vec<Game> = (1..=40)
            .map(|id| game(id, f64::from(id) / 100.0, 1))
            .collect();
        let Plan::Hours(together) = plan(&library(games), &Preferences::default(), &[], now())
        else {
            panic!("not playing them together");
        };
        assert_eq!(together.len(), 32);
        assert_eq!(together[0], AppId(40), "the most hours first");
    }

    #[test]
    fn a_priority_building_hours_leads_the_group_ahead_of_games_that_can_drop() {
        let lib = library(vec![game(1, 5.0, 2), game(2, 1.0, 3), game(3, 2.0, 1)]);
        assert_eq!(
            plan(&lib, &priorities(&[2]), &[], now()),
            Plan::Hours(ids(&[2, 3])),
            "the priority first, then the others building hours"
        );
    }

    #[test]
    fn nothing_says_why() {
        let prefs = Preferences::default();
        assert_eq!(
            plan(&SteamLibrary::default(), &prefs, &[], now()),
            Plan::Nothing
        );
        assert_eq!(
            why_nothing(&SteamLibrary::default(), &prefs, now()),
            NothingToFarm::NoGames
        );
        let done = library(vec![game(1, 5.0, 0)]);
        assert_eq!(why_nothing(&done, &prefs, now()), NothingToFarm::AllDropped);
        let skipped = Preferences {
            skipped_games: ids(&[1]),
            ..Default::default()
        };
        let left = library(vec![game(1, 5.0, 2)]);
        assert_eq!(
            why_nothing(&left, &skipped, now()),
            NothingToFarm::AllSkipped
        );
    }

    #[test]
    fn private_games_are_left_out_unless_the_user_farms_them_too() {
        let private = Game {
            private: true,
            ..game(1, 5.0, 2)
        };
        let lib = library(vec![private.clone(), game(2, 5.0, 3)]);
        let prefs = priorities(&[1]);
        assert_eq!(
            left_out(&private, &prefs, now()),
            Some(LeftOut::Private),
            "a priority too: Steam drops nothing for it"
        );
        assert_eq!(farm_order(&lib, &prefs, &[], now()), ids(&[2]));

        let farmed_too = Preferences {
            skip_private: false,
            ..prefs
        };
        assert_eq!(left_out(&private, &farmed_too, now()), None);
        assert_eq!(farm_order(&lib, &farmed_too, &[], now()), ids(&[1, 2]));
    }

    #[test]
    fn a_game_steam_would_still_refund_waits_until_it_wouldnt() {
        let bought = Game {
            bought_at: Some(now() - TimeDelta::days(3)),
            ..game(1, 0.5, 2)
        };
        let lib = library(vec![bought.clone(), game(2, 5.0, 3)]);
        let prefs = Preferences::default();
        let until = now() + TimeDelta::days(11);
        assert_eq!(
            left_out(&bought, &prefs, now()),
            Some(LeftOut::Refundable { until })
        );
        assert_eq!(farm_order(&lib, &prefs, &[], now()), ids(&[2]));
        assert_eq!(first_refund_ends(&lib, &prefs, now()), Some(until));
        assert_eq!(
            farm_order(&lib, &prefs, &[], until),
            ids(&[2, 1]),
            "once Steam wouldn't refund it, it's farmed"
        );

        let played = Game {
            hours: 2.0,
            ..bought.clone()
        };
        assert_eq!(left_out(&played, &prefs, now()), None, "played 2 hours");

        let farmed_too = Preferences {
            skip_refundable: false,
            ..Default::default()
        };
        assert_eq!(left_out(&bought, &farmed_too, now()), None);
        assert_eq!(first_refund_ends(&lib, &farmed_too, now()), None);
    }

    #[test]
    fn the_users_own_choices_say_why_before_steams() {
        let both = Game {
            private: true,
            ..game(1, 5.0, 2)
        };
        let skipped = Preferences {
            skipped_games: ids(&[1]),
            ..Default::default()
        };
        assert_eq!(left_out(&both, &skipped, now()), Some(LeftOut::Skipped));
        let only = Preferences {
            only_priority: true,
            ..Default::default()
        };
        assert_eq!(left_out(&both, &only, now()), Some(LeftOut::NotPriority));
        assert_eq!(
            left_out(&game(2_861_720, 0.0, 3), &Preferences::default(), now()),
            Some(LeftOut::SaleEvent)
        );
    }

    #[test]
    fn the_hours_a_game_needs_are_the_accounts() {
        let lib = library(vec![game(1, 2.5, 2), game(2, 1.0, 3)]);
        let two = Preferences {
            hours_before_drops: 2,
            ..Default::default()
        };
        assert_eq!(plan(&lib, &two, &[], now()), Plan::Cards(AppId(1)));
        assert_eq!(
            plan(&lib, &Preferences::default(), &[], now()),
            Plan::Hours(ids(&[1, 2])),
            "short of 3, both build hours"
        );
    }

    #[test]
    fn on_an_account_steam_doesnt_hold_back_every_game_is_farmed_alone() {
        let lib = library(vec![game(1, 0.0, 2), game(2, 0.5, 3)]);
        let none = Preferences {
            hours_before_drops: 0,
            ..Default::default()
        };
        assert_eq!(
            farm_order(&lib, &none, &[], now()),
            ids(&[1, 2]),
            "fewest drops left first"
        );
        assert_eq!(plan(&lib, &none, &[], now()), Plan::Cards(AppId(1)));
    }

    #[test]
    fn nothing_to_farm_for_steams_reasons_says_so() {
        let private = Game {
            private: true,
            ..game(1, 5.0, 2)
        };
        let bought = Game {
            bought_at: Some(now() - TimeDelta::days(13)),
            ..game(2, 0.0, 3)
        };
        let prefs = Preferences {
            skipped_games: ids(&[3]),
            ..Default::default()
        };
        assert_eq!(
            why_nothing(
                &library(vec![private.clone(), game(3, 1.0, 1)]),
                &prefs,
                now()
            ),
            NothingToFarm::HeldBack {
                private: true,
                refundable_until: None
            },
            "the game the user skipped isn't Steam's doing, and goes unsaid"
        );
        assert_eq!(
            why_nothing(&library(vec![private, bought]), &prefs, now()),
            NothingToFarm::HeldBack {
                private: true,
                refundable_until: Some(now() + TimeDelta::days(1))
            }
        );
        assert_eq!(
            why_nothing(
                &library(vec![game(2_861_720, 0.0, 3), game(3, 1.0, 1)]),
                &prefs,
                now()
            ),
            NothingToFarm::NotDropping,
            "a sale event's badge"
        );
    }
}
