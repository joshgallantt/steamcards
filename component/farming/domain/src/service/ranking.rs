//! What to play, and how. Pure: the library, the preferences and what's been
//! set aside go in; an order and a plan come out.

use game::{AppId, Game, MOST_PLAYED_AT_ONCE, SteamLibrary};
use preferences::Preferences;
use session::SetAside;

use crate::{
    NothingToFarm,
    model::{
        Plan,
        rules::{GIVE_UP_TIMES, SALE_EVENTS},
    },
};

/// The games worth farming, by app ID, in the order they're farmed:
///
/// 1. the user's priority games, in their order;
/// 2. games whose cards can drop now, fewest drops left first, so games
///    finish sooner (as Steam Game Idler orders them);
/// 3. games still building hours, most hours first: the closest to dropping;
/// 4. games set aside after a long while without a drop, last.
///
/// Left out: finished games; games the user doesn't want (skipped, or not a
/// priority with "only priority" on); sale-event badges, which playing never
/// drops; and games set aside too often this run.
///
/// Public so a screen can size up the job before farming starts, in the
/// order the farmer will take it.
pub fn farm_order(
    library: &SteamLibrary,
    prefs: &Preferences,
    set_aside: &[SetAside],
) -> Vec<AppId> {
    let aside = |g: &Game| {
        set_aside
            .iter()
            .find(|s| s.app_id == g.app_id)
            .map_or(0, |s| s.times)
    };
    let mut games: Vec<&Game> = library
        .with_drops_left()
        .filter(|g| prefs.wants(g.app_id) && !SALE_EVENTS.contains(&g.app_id))
        .filter(|g| aside(g) < GIVE_UP_TIMES)
        .collect();
    games.sort_by(|a, b| {
        let rank = |g: &Game| prefs.rank(g.app_id).unwrap_or(usize::MAX);
        aside(a)
            .cmp(&aside(b))
            .then(rank(a).cmp(&rank(b)))
            .then(b.can_drop().cmp(&a.can_drop()))
            .then_with(|| {
                if a.can_drop() && b.can_drop() {
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
/// Game Idler since 6.2).
pub(crate) fn plan(library: &SteamLibrary, prefs: &Preferences, set_aside: &[SetAside]) -> Plan {
    let order = farm_order(library, prefs, set_aside);
    let Some(first) = order.first().and_then(|&id| library.game(id)) else {
        return Plan::Nothing;
    };
    if first.can_drop() {
        return Plan::Cards(first.app_id);
    }
    Plan::Hours(
        order
            .iter()
            .copied()
            .filter(|&id| library.game(id).is_some_and(|g| !g.can_drop()))
            .take(MOST_PLAYED_AT_ONCE)
            .collect(),
    )
}

/// Why nothing is being farmed, in the user's words.
pub(crate) fn why_nothing(library: &SteamLibrary, prefs: &Preferences) -> NothingToFarm {
    if library.is_empty() {
        NothingToFarm::NoGames
    } else if library.drops_left() == 0 {
        NothingToFarm::AllDropped
    } else if library.with_drops_left().all(|g| !prefs.wants(g.app_id)) {
        if prefs.only_priority {
            NothingToFarm::PrioritiesDone
        } else {
            NothingToFarm::AllSkipped
        }
    } else {
        NothingToFarm::NotDropping
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use game::CardDrops;

    use super::*;

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
        let order = farm_order(&lib, &Preferences::default(), &[]);
        assert_eq!(order, ids(&[3, 1, 4, 2]), "then the most hours first");
    }

    #[test]
    fn the_users_priorities_come_before_everything() {
        let lib = library(vec![game(1, 5.0, 4), game(2, 0.5, 3), game(3, 8.0, 1)]);
        let order = farm_order(&lib, &priorities(&[2, 1]), &[]);
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
        assert_eq!(farm_order(&lib, &prefs, &[]), ids(&[5, 1]));

        let only = Preferences {
            priority_games: ids(&[1]),
            only_priority: true,
            ..Default::default()
        };
        assert_eq!(farm_order(&lib, &only, &[]), ids(&[1]));
    }

    #[test]
    fn a_game_set_aside_waits_behind_the_rest_then_is_left_alone() {
        let lib = library(vec![game(1, 5.0, 1), game(2, 5.0, 2)]);
        let once = [aside(1, 1)];
        assert_eq!(
            farm_order(&lib, &Preferences::default(), &once),
            ids(&[2, 1])
        );
        let twice = [aside(1, 2)];
        assert_eq!(farm_order(&lib, &Preferences::default(), &twice), ids(&[2]));
    }

    #[test]
    fn a_game_that_can_drop_is_played_alone() {
        let lib = library(vec![game(1, 5.0, 2), game(2, 1.0, 3)]);
        assert_eq!(
            plan(&lib, &Preferences::default(), &[]),
            Plan::Cards(AppId(1))
        );
    }

    #[test]
    fn games_building_hours_are_played_together_up_to_32() {
        let games: Vec<Game> = (1..=40)
            .map(|id| game(id, f64::from(id) / 100.0, 1))
            .collect();
        let Plan::Hours(together) = plan(&library(games), &Preferences::default(), &[]) else {
            panic!("not playing them together");
        };
        assert_eq!(together.len(), 32);
        assert_eq!(together[0], AppId(40), "the most hours first");
    }

    #[test]
    fn a_priority_building_hours_leads_the_group_ahead_of_games_that_can_drop() {
        let lib = library(vec![game(1, 5.0, 2), game(2, 1.0, 3), game(3, 2.0, 1)]);
        assert_eq!(
            plan(&lib, &priorities(&[2]), &[]),
            Plan::Hours(ids(&[2, 3])),
            "the priority first, then the others building hours"
        );
    }

    #[test]
    fn nothing_says_why() {
        let prefs = Preferences::default();
        assert_eq!(plan(&SteamLibrary::default(), &prefs, &[]), Plan::Nothing);
        assert_eq!(
            why_nothing(&SteamLibrary::default(), &prefs),
            NothingToFarm::NoGames
        );
        let done = library(vec![game(1, 5.0, 0)]);
        assert_eq!(why_nothing(&done, &prefs), NothingToFarm::AllDropped);
        let skipped = Preferences {
            skipped_games: ids(&[1]),
            ..Default::default()
        };
        let left = library(vec![game(1, 5.0, 2)]);
        assert_eq!(why_nothing(&left, &skipped), NothingToFarm::AllSkipped);
    }
}
