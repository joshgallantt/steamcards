//! How long farming should take to finish, learnt from this session's drops.
//! Pure: the session, the library and the farm order go in; a forecast comes
//! out.
//!
//! Most cards drop about every half hour, but not like clockwork, and each
//! game's developer sets its pace. So the rate is learnt as farming goes,
//! with Gamma–Poisson shrinkage (research: market-and-session.md, section
//! 3.1):
//!
//! - K drops in T hours of farming alone, the stretch going on counted up to
//!   now. Building hours, waiting and being paused teach it nothing, and
//!   drops found then don't count.
//! - The account's rate: r = (2 + K) / (1 h + T), which is ASF's 30 minutes
//!   a drop until a few real drops outweigh it.
//! - A game's rate: r_g = (2 + k_g) / (2/r + T_g), from its own drops once
//!   it has been farmed alone; before that, the account's.
//! - The time to finish: Σ N_g / r_g over the drops N_g each game has left,
//!   plus ASF's hours term (`CardsFarmer.TimeRemaining`, Apache-2.0): games
//!   short of 3 hours are played together, 32 at a time, and each group
//!   takes 3 hours less its lowest hours.
//! - Its 80% band: × e^(±1.28 √(1/N + 1/(2 + K))), where N is Σ N_g.
//!
//! Before the second drop there's nothing to learn from: it assumes 30
//! minutes a drop, and gives no band.

use std::time::Duration;

use chrono::{DateTime, Utc};
use library::SteamLibrary;

use crate::{
    Drop, FarmingSession, Forecast, Mode, Stretch,
    ranking::hours_to_go,
    rules::{BAND_80, MOST_AT_ONCE, PRIOR_DROPS, PRIOR_HOURS},
};

/// How long farming the games in `order` should take from `now`, learnt from
/// `session`'s drops farming alone. `order` is the farm order: games not in
/// it, or with no drops left, aren't farmed, so they take no time.
pub fn forecast(
    session: &FarmingSession,
    library: &SteamLibrary,
    order: &[u32],
    now: DateTime<Utc>,
) -> Forecast {
    let alone: Vec<&Stretch> = session
        .stretches
        .iter()
        .filter(|s| s.mode == Mode::Cards)
        .collect();
    // Drops, and hours, farming alone: all of them, or one game's.
    let k = |app: Option<u32>| {
        session
            .drops
            .iter()
            .filter(|d| app.is_none_or(|a| d.app_id == a))
            .filter(|d| alone.iter().any(|s| during(d, s, now)))
            .count() as f64
    };
    let t = |app: Option<u32>| {
        alone
            .iter()
            .filter(|s| app.is_none_or(|a| s.app_ids.contains(&a)))
            .map(|s| hours(s, now))
            .sum::<f64>()
    };

    let learnt_from = k(None);
    let assumed = learnt_from < 2.0;
    let rate = if assumed {
        PRIOR_DROPS / PRIOR_HOURS
    } else {
        (PRIOR_DROPS + learnt_from) / (PRIOR_HOURS + t(None))
    };
    let rate_of = |app: u32| {
        if assumed {
            rate
        } else {
            (PRIOR_DROPS + k(Some(app))) / (PRIOR_DROPS / rate + t(Some(app)))
        }
    };

    let mut elapsed = 0.0;
    let mut hours_term = 0.0;
    let mut left = 0;
    // The group building hours: how many games are in it, and the hours it
    // has built so far.
    let mut group = (0, 0.0);
    let mut per_game = Vec::new();
    for game in order
        .iter()
        .filter_map(|&id| library.game(id))
        .filter(|g| g.has_drops_left())
    {
        let to_go = hours_to_go(game);
        if to_go > 0.0 {
            if group.0 == MOST_AT_ONCE {
                group = (0, 0.0);
            }
            // Built alongside the ones before it, it needs only what they
            // didn't need: the group takes as long as its lowest needs.
            let more = (to_go - group.1).max(0.0);
            group = (group.0 + 1, group.1 + more);
            hours_term += more;
            elapsed += more;
        }
        elapsed += f64::from(game.drops.remaining) / rate_of(game.app_id);
        left += game.drops.remaining;
        per_game.push((game.app_id, span(elapsed)));
    }

    let band = (!assumed && left > 0).then(|| {
        let spread =
            (BAND_80 * (1.0 / f64::from(left) + 1.0 / (PRIOR_DROPS + learnt_from)).sqrt()).exp();
        (span(elapsed / spread), span(elapsed * spread))
    });
    Forecast {
        eta: span(elapsed),
        band,
        assumed,
        hours_term: span(hours_term),
        rate,
        per_game,
        made_at: now,
    }
}

/// Whether a drop came while its game was farmed alone in `s`.
fn during(d: &Drop, s: &Stretch, now: DateTime<Utc>) -> bool {
    s.app_ids.contains(&d.app_id) && s.from <= d.at && d.at <= s.to.unwrap_or(now)
}

/// Hours `s` has lasted, up to `now` if it goes on.
fn hours(s: &Stretch, now: DateTime<Utc>) -> f64 {
    let lasted = s.to.unwrap_or(now) - s.from;
    (lasted.num_milliseconds() as f64 / 3_600_000.0).max(0.0)
}

/// `hours` as a duration.
fn span(hours: f64) -> Duration {
    Duration::try_from_secs_f64(hours * 3600.0).unwrap_or(Duration::MAX)
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use library::{CardDrops, Game};

    use super::*;
    use crate::DropCard;

    fn at(hours: f64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 29, 9, 0, 0).unwrap()
            + chrono::Duration::milliseconds((hours * 3_600_000.0) as i64)
    }

    fn game(app_id: u32, hours: f64, remaining: u32) -> Game {
        Game {
            app_id,
            name: format!("Game {app_id}"),
            hours,
            drops: CardDrops {
                received: 0,
                remaining,
            },
            badge_level: 0,
            cards: Vec::new(),
        }
    }

    fn stretch(app_ids: &[u32], mode: Mode, from: f64, to: Option<f64>) -> Stretch {
        Stretch {
            app_ids: app_ids.to_vec(),
            mode,
            from: at(from),
            to: to.map(at),
        }
    }

    /// `n` drops of `app_id`, evenly between `from` and `to` hours.
    fn drops(app_id: u32, n: u32, from: f64, to: f64) -> Vec<Drop> {
        (1..=n)
            .map(|i| Drop {
                at: at(from + (to - from) * f64::from(i) / f64::from(n)),
                app_id,
                card: DropCard::Unknown,
                copy: None,
            })
            .collect()
    }

    fn hours_of(d: Duration) -> f64 {
        d.as_secs_f64() / 3600.0
    }

    fn near(actual: f64, expected: f64, within: f64) -> bool {
        (actual - expected).abs() <= within
    }

    #[test]
    fn six_drops_in_two_and_a_half_hours_learn_the_rate() {
        // The research's worked example: 6 drops in 2.5 hours farming one
        // game alone, and 40 drops left in games never farmed alone.
        let session = FarmingSession {
            drops: drops(1, 6, 0.0, 2.5),
            stretches: vec![stretch(&[1], Mode::Cards, 0.0, Some(2.5))],
            ..Default::default()
        };
        let library = SteamLibrary::new(vec![game(1, 8.0, 0), game(2, 5.0, 25), game(3, 4.0, 15)]);

        let f = forecast(&session, &library, &[2, 3], at(2.5));

        assert!(!f.assumed);
        assert!(near(f.rate, 2.29, 0.01), "about 2.3 an hour: {}", f.rate);
        assert!(near(hours_of(f.eta), 17.5, 0.01), "{:?}", f.eta);
        let (low, high) = f.band.expect("a band, with two drops or more");
        assert!(near(hours_of(low), 10.7, 0.05), "{low:?}");
        assert!(near(hours_of(high), 28.7, 0.05), "{high:?}");
        assert_eq!(f.hours_term, Duration::ZERO, "every game has 3 hours");
        assert_eq!(f.per_game[0].0, 2);
        assert!(near(hours_of(f.per_game[0].1), 25.0 / f.rate, 0.01));
        assert_eq!(f.per_game[1].0, 3);
        assert_eq!(
            f.per_game[1].1, f.eta,
            "the last game's is the time to finish"
        );
        assert_eq!(f.made_at, at(2.5));
    }

    #[test]
    fn before_the_second_drop_it_assumes_30_minutes_a_drop() {
        let library = SteamLibrary::new(vec![game(1, 5.0, 4), game(2, 5.0, 6)]);
        let nothing_yet = FarmingSession::default();
        let one_slow_drop = FarmingSession {
            drops: drops(1, 1, 0.0, 2.0),
            stretches: vec![stretch(&[1], Mode::Cards, 0.0, None)],
            ..Default::default()
        };

        for session in [nothing_yet, one_slow_drop] {
            let f = forecast(&session, &library, &[1, 2], at(2.0));
            assert!(f.assumed);
            assert_eq!(f.rate, 2.0, "30 minutes a drop");
            assert_eq!(f.eta, Duration::from_secs(5 * 3600), "10 drops left");
            assert_eq!(f.band, None, "no band on an assumption");
        }
    }

    #[test]
    fn only_farming_alone_teaches_the_rate() {
        let library = SteamLibrary::new(vec![game(1, 5.0, 4)]);
        // Three drops while building hours, and one found after a pause.
        let session = FarmingSession {
            drops: [drops(2, 3, 0.0, 1.0), drops(1, 1, 3.0, 3.0)].concat(),
            stretches: vec![
                stretch(&[2, 3], Mode::Hours, 0.0, Some(1.0)),
                stretch(&[1], Mode::Cards, 1.0, Some(2.0)),
            ],
            ..Default::default()
        };

        let f = forecast(&session, &library, &[1], at(3.0));

        assert!(f.assumed, "none of them came farming alone");
    }

    #[test]
    fn the_stretch_going_on_counts_up_to_now() {
        let library = SteamLibrary::new(vec![game(1, 5.0, 6)]);
        let session = FarmingSession {
            drops: drops(1, 4, 0.0, 1.5),
            stretches: vec![stretch(&[1], Mode::Cards, 0.0, None)],
            ..Default::default()
        };

        let f = forecast(&session, &library, &[1], at(2.0));

        assert!(near(f.rate, 2.0, 1e-9), "(2 + 4) / (1 + 2): {}", f.rate);
    }

    #[test]
    fn a_game_farmed_alone_goes_at_its_own_pace() {
        // Game 1 dropped 2 in 4 hours; game 2, 4 in an hour.
        let session = FarmingSession {
            drops: [drops(1, 2, 0.0, 4.0), drops(2, 4, 4.0, 5.0)].concat(),
            stretches: vec![
                stretch(&[1], Mode::Cards, 0.0, Some(4.0)),
                stretch(&[2], Mode::Cards, 4.0, None),
            ],
            ..Default::default()
        };
        let library = SteamLibrary::new(vec![game(1, 9.0, 2), game(2, 6.0, 2), game(3, 7.0, 2)]);

        let f = forecast(&session, &library, &[2, 1, 3], at(5.0));

        let r = 8.0 / 6.0;
        assert!(near(f.rate, r, 1e-9));
        let slow = 4.0 / (2.0 / r + 4.0);
        let fast = 6.0 / (2.0 / r + 1.0);
        let times: Vec<f64> = f.per_game.iter().map(|(_, d)| hours_of(*d)).collect();
        assert!(near(times[0], 2.0 / fast, 1e-6), "{times:?}");
        assert!(near(times[1], 2.0 / fast + 2.0 / slow, 1e-6));
        assert!(
            near(times[2], 2.0 / fast + 2.0 / slow + 2.0 / r, 1e-6),
            "a game not farmed yet goes at the account's rate"
        );
    }

    #[test]
    fn games_short_of_three_hours_add_the_hours_they_build() {
        let library = SteamLibrary::new(vec![game(1, 5.0, 2), game(2, 2.0, 1), game(3, 0.5, 1)]);

        let f = forecast(&FarmingSession::default(), &library, &[1, 2, 3], at(0.0));

        assert_eq!(f.hours_term, Duration::from_secs(9000), "3 hours less 0.5");
        assert_eq!(f.eta, Duration::from_secs(4 * 3600 + 1800));
        let times: Vec<f64> = f.per_game.iter().map(|(_, d)| hours_of(*d)).collect();
        assert_eq!(
            times,
            [1.0, 1.0 + 1.0 + 0.5, 2.5 + 1.5 + 0.5],
            "each built only past the one before it"
        );
    }

    #[test]
    fn hours_are_built_32_games_at_a_time() {
        let games: Vec<Game> = (1..=33).map(|id| game(id, 0.0, 1)).collect();
        let order: Vec<u32> = (1..=33).collect();

        let f = forecast(
            &FarmingSession::default(),
            &SteamLibrary::new(games),
            &order,
            at(0.0),
        );

        assert_eq!(f.hours_term, Duration::from_secs(6 * 3600), "two groups");
    }

    #[test]
    fn games_not_to_be_farmed_take_no_time() {
        let library = SteamLibrary::new(vec![game(1, 5.0, 2), game(2, 5.0, 0), game(3, 1.0, 4)]);

        let f = forecast(&FarmingSession::default(), &library, &[1, 2], at(0.0));

        assert_eq!(
            f.per_game.len(),
            1,
            "game 2 is done, game 3 not in the order"
        );
        assert_eq!(f.eta, Duration::from_secs(3600));
        let done = forecast(&FarmingSession::default(), &library, &[], at(0.0));
        assert_eq!(done.eta, Duration::ZERO);
        assert!(done.per_game.is_empty());
    }
}
