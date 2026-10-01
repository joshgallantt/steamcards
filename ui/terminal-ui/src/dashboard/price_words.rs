//! What the background pricing reports, in the user's words, for the log:
//! a game by its name when the library knows it, a wait in minutes, and the
//! next round in hours and minutes.

use std::time::Duration;

use card::PriceEvent;
use game::SteamLibrary;

use crate::dashboard::EventKind;

/// The log's line for what the pricing reported, and what it's about.
pub(crate) fn line(event: &PriceEvent, library: &SteamLibrary) -> (EventKind, String) {
    match event {
        PriceEvent::AllPriced {
            games,
            next_round,
            at,
        } => {
            let games = match games {
                1 => "the 1 game".to_owned(),
                n => format!("all {n} games"),
            };
            let next = next_round.map_or_else(String::new, |next| {
                let wait = (next - *at).to_std().unwrap_or_default();
                format!("; the next round is in {}", in_words(wait))
            });
            (
                EventKind::Progress,
                format!("Prices: {games} looked up{next}."),
            )
        }
        PriceEvent::Paused { pause, again } => {
            let wait = minutes(pause.step);
            let line = if *again {
                format!(
                    "Steam turned down price lookups again: they wait {wait} now. Farming carries on."
                )
            } else {
                format!(
                    "Steam turned down a price lookup: lookups wait {wait}. Farming carries on."
                )
            };
            (EventKind::Warning, line)
        }
        PriceEvent::Resumed => (
            EventKind::Info,
            "Steam's pause on price lookups is over: they carry on.".into(),
        ),
        PriceEvent::Failed { app_id, why } => {
            let game = library
                .game(*app_id)
                .map_or_else(|| format!("app {app_id}"), |g| g.name.clone());
            (
                EventKind::Warning,
                format!(
                    "Couldn't look up the prices of {game}'s cards: {why}. They're tried again \
                     in 24 hours."
                ),
            )
        }
        PriceEvent::Unanswered { why, wait, .. } => (
            EventKind::Progress,
            format!(
                "Couldn't ask the market for prices: {why}. Asking again in {}.",
                minutes(*wait)
            ),
        ),
    }
}

/// "10 minutes", "an hour".
fn minutes(d: Duration) -> String {
    match d.as_secs() / 60 {
        60 => "an hour".into(),
        1 => "a minute".into(),
        m => format!("{m} minutes"),
    }
}

/// A duration in two units at most, as the screens write them: "<1m",
/// "38m", "5h 58m", "4d 21h".
fn in_words(d: Duration) -> String {
    let minutes = d.as_secs() / 60;
    let (days, hours, minutes) = (minutes / (24 * 60), minutes / 60 % 24, minutes % 60);
    match (days, hours, minutes) {
        (0, 0, 0) => "<1m".into(),
        (0, 0, m) => format!("{m}m"),
        (0, h, 0) => format!("{h}h"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, 0, _) => format!("{d}d"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

#[cfg(test)]
mod tests {
    use card::MarketPause;
    use chrono::{DateTime, TimeDelta, Utc};
    use game::{AppId, test_support::game};

    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    fn at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T09:14:00Z")
            .unwrap()
            .to_utc()
    }

    fn said(event: PriceEvent) -> (EventKind, String) {
        let mut hades = game(1_145_360, 12.5, 3, 1);
        hades.name = "Hades".into();
        line(&event, &SteamLibrary::new(vec![hades]))
    }

    #[test]
    fn a_round_of_pricing_says_how_many_games_and_when_the_next_is() {
        let in_6h = Some(at() + TimeDelta::hours(6));
        assert_eq!(
            said(PriceEvent::AllPriced {
                games: 3,
                next_round: in_6h,
                at: at(),
            }),
            (
                EventKind::Progress,
                "Prices: all 3 games looked up; the next round is in 6h.".into()
            )
        );
        assert_eq!(
            said(PriceEvent::AllPriced {
                games: 1,
                next_round: None,
                at: at(),
            })
            .1,
            "Prices: the 1 game looked up."
        );
    }

    #[test]
    fn a_pause_says_how_long_lookups_wait_and_whether_its_another() {
        let pause = |minutes| MarketPause {
            until: at(),
            step: minutes * MINUTE,
        };
        assert_eq!(
            said(PriceEvent::Paused {
                pause: pause(10),
                again: false,
            }),
            (
                EventKind::Warning,
                "Steam turned down a price lookup: lookups wait 10 minutes. Farming carries on."
                    .into()
            )
        );
        assert_eq!(
            said(PriceEvent::Paused {
                pause: pause(20),
                again: true,
            })
            .1,
            "Steam turned down price lookups again: they wait 20 minutes now. Farming carries on."
        );
        assert_eq!(
            said(PriceEvent::Resumed),
            (
                EventKind::Info,
                "Steam's pause on price lookups is over: they carry on.".into()
            )
        );
    }

    #[test]
    fn a_failed_lookup_names_the_game_when_the_library_does() {
        let failed = |app_id| PriceEvent::Failed {
            app_id: AppId(app_id),
            why: "steamcommunity.com's market said 502 Bad Gateway, twice".into(),
        };
        assert_eq!(
            said(failed(1_145_360)),
            (
                EventKind::Warning,
                "Couldn't look up the prices of Hades's cards: steamcommunity.com's market said \
                 502 Bad Gateway, twice. They're tried again in 24 hours."
                    .into()
            )
        );
        assert_eq!(
            said(failed(620)).1,
            "Couldn't look up the prices of app 620's cards: steamcommunity.com's market said \
             502 Bad Gateway, twice. They're tried again in 24 hours.",
            "a game the library doesn't know by its app ID"
        );
    }

    #[test]
    fn a_market_that_couldnt_be_asked_says_when_its_asked_again() {
        let unanswered = |wait| PriceEvent::Unanswered {
            why: "couldn't sign on to Steam: no network".into(),
            wait,
            retry_at: at(),
        };
        assert_eq!(
            said(unanswered(MINUTE)),
            (
                EventKind::Progress,
                "Couldn't ask the market for prices: couldn't sign on to Steam: no network. \
                 Asking again in a minute."
                    .into()
            )
        );
        assert_eq!(
            said(unanswered(4 * MINUTE)).1,
            "Couldn't ask the market for prices: couldn't sign on to Steam: no network. Asking \
             again in 4 minutes."
        );
    }

    #[test]
    fn durations_read_as_the_screens_write_them() {
        assert_eq!(in_words(Duration::from_secs(20)), "<1m");
        assert_eq!(in_words(38 * MINUTE), "38m");
        assert_eq!(in_words(6 * 60 * MINUTE), "6h");
        assert_eq!(in_words(358 * MINUTE), "5h 58m");
        assert_eq!(in_words((4 * 24 + 21) * 60 * MINUTE + 5 * MINUTE), "4d 21h");
        assert_eq!(in_words(2 * 24 * 60 * MINUTE), "2d");
        assert_eq!(minutes(10 * MINUTE), "10 minutes");
        assert_eq!(minutes(60 * MINUTE), "an hour");
    }
}
