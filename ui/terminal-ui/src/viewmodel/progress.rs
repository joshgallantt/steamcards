// Progress: how far through the library, when it should be done, and what the
// cards are worth (docs/design/ui.md §2.1, §5.2). Built fresh each frame
// from the farmer's status and forecast, the library and the market.

use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use farming::{Drop, FarmingSession, Mode};
use market::{Held, MarketPause, Money};
use preferences::Tier;

use super::screen::{Activity, Snapshot, Values, session_value};

/// The Progress panel, and the glance the too-small screen keeps.
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub activity: Activity,
    /// The time to finish; `None` until the badges are read.
    pub eta: Option<Eta>,
    pub session: SessionProgress,
    pub library: LibraryProgress,
    pub to_go: ToGo,
    /// What the cards are worth; `None` until Steam says which currency the
    /// wallet is in.
    pub values: Option<Values>,
    /// Steam's pause on price lookups, while it lasts.
    pub pause: Option<MarketPause>,
    /// The session's summary, once there's nothing left to farm.
    pub summary: Option<Summary>,
    pub now: DateTime<Utc>,
    pub zone: FixedOffset,
}

/// The time to finish, and what it was learnt from.
#[derive(Debug, Clone, PartialEq)]
pub struct Eta {
    pub eta: Duration,
    /// Where it falls 80% of the time; `None` before the second drop.
    pub band: Option<(Duration, Duration)>,
    /// When it lands, if left running: "around Sun 4 Oct".
    pub lands: DateTime<Utc>,
    /// Too few drops yet: it assumes 30 minutes a drop.
    pub assumed: bool,
    /// The part of it spent building hours: "incl. ≈ 3h of building hours".
    pub hours_term: Duration,
    /// Drops an hour, farming alone.
    pub rate: f64,
    /// What the rate was learnt from: "16 in 7h 40m farming alone".
    pub learnt: Learnt,
    /// Farming time stands still, so this does too: "of farming left", no
    /// date.
    pub holds_still: bool,
}

/// Drops that came farming alone, and how long that was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Learnt {
    pub drops: u32,
    pub alone: Duration,
}

/// This session's drops and games, out of those at the start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionProgress {
    pub drops: u32,
    /// Drops left in the games it farms, at the start; `None` until the
    /// badges are read.
    pub drops_at_start: Option<u32>,
    pub games_done: usize,
    pub games_at_start: Option<u32>,
    pub started_at: Option<DateTime<Utc>>,
}

/// The whole library's drops and games.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryProgress {
    pub received: u32,
    pub total: u32,
    pub games_done: usize,
    pub games: usize,
}

/// What's still to farm: the games in the farm order, their drops, and how
/// many of them are set aside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToGo {
    pub games: usize,
    pub drops: u32,
    pub set_aside: usize,
}

/// A session that has farmed everything, summed up (mockup e): what it did,
/// how its first estimate did, and what the cards are worth.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub from: DateTime<Utc>,
    /// When the last card dropped.
    pub to: DateTime<Utc>,
    pub drops: u32,
    pub games: usize,
    pub rate: Option<f64>,
    /// The first estimate with a likely range, and how it did.
    pub estimate: Option<Checked>,
    /// This session's cards on the basis.
    pub value: Option<Held>,
    /// The foils that dropped, by name.
    pub foils: Vec<String>,
    /// What the first estimate said the cards would be worth, as the screen
    /// kept it.
    pub estimated: Option<Money>,
    /// When the farmer looks again.
    pub next_look: Option<DateTime<Utc>>,
    /// Why nothing is left: "all done", "all done or skipped", or the
    /// farmer's own reason.
    pub why: String,
}

/// The first estimate with a likely range, against what happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checked {
    pub said: Duration,
    pub band: Option<(Duration, Duration)>,
    pub made_at: DateTime<Utc>,
    /// How long it took from then.
    pub took: Duration,
    pub inside: bool,
}

impl Progress {
    /// `estimated` is what the session's first estimate said the cards
    /// would be worth on completion, which the screen keeps as it's made.
    pub fn build(s: &Snapshot<'_>, estimated: Option<Money>) -> Self {
        let activity = Activity::of(s);
        let library = s.library();
        let session = s.session();
        let order = s.order();
        let to_go = ToGo {
            games: order.len(),
            drops: order
                .iter()
                .filter_map(|&id| library.game(id))
                .map(|g| g.drops.remaining)
                .sum(),
            set_aside: s.status.map_or(0, |st| {
                st.set_aside
                    .iter()
                    .filter(|a| order.contains(&a.app_id))
                    .count()
            }),
        };
        let eta = s.forecast.filter(|_| !library.is_empty()).map(|f| Eta {
            eta: f.eta,
            band: f.band,
            lands: s.now + f.eta,
            assumed: f.assumed,
            hours_term: f.hours_term,
            rate: f.rate,
            learnt: session.map_or(
                Learnt {
                    drops: 0,
                    alone: Duration::ZERO,
                },
                |session| learnt(session, s.now),
            ),
            holds_still: activity.holds_still(),
        });
        let summary = match (&activity, session) {
            (Activity::NothingToFarm { why, next_look }, Some(session))
                if !session.drops.is_empty() =>
            {
                Some(Summary {
                    why: why_nothing(s, why),
                    ..summary(s, session, estimated, *next_look)
                })
            }
            _ => None,
        };
        Self {
            eta,
            session: SessionProgress {
                drops: session.map_or(0, |ss| u32::try_from(ss.drops.len()).unwrap_or(u32::MAX)),
                drops_at_start: session.and_then(|ss| ss.drops_left_at_start),
                games_done: session.map_or(0, |ss| ss.finished.len()),
                games_at_start: session.and_then(|ss| ss.games_at_start),
                started_at: session.map(|ss| ss.started_at),
            },
            library: LibraryProgress {
                received: library.drops_received(),
                total: library.drops_total(),
                games_done: library.games_done(),
                games: library.games().len(),
            },
            to_go,
            values: Values::build(s),
            pause: s.pause.filter(|p| p.until > s.now),
            summary,
            activity,
            now: s.now,
            zone: s.zone,
        }
    }
}

/// The drops that came while their game was farmed alone, and how long that
/// was, the stretch going on counted up to `now`: what the rate is learnt
/// from, as the forecast learns it.
pub fn learnt(session: &FarmingSession, now: DateTime<Utc>) -> Learnt {
    let alone: Vec<_> = session
        .stretches
        .iter()
        .filter(|s| s.mode == Mode::Cards)
        .collect();
    let during = |d: &Drop| {
        alone
            .iter()
            .any(|s| s.app_ids.contains(&d.app_id) && s.from <= d.at && d.at <= s.to.unwrap_or(now))
    };
    Learnt {
        drops: u32::try_from(session.drops.iter().filter(|d| during(d)).count())
            .unwrap_or(u32::MAX),
        alone: alone
            .iter()
            .map(|s| (s.to.unwrap_or(now) - s.from).to_std().unwrap_or_default())
            .sum(),
    }
}

fn summary(
    s: &Snapshot<'_>,
    session: &FarmingSession,
    estimated: Option<Money>,
    next_look: Option<DateTime<Utc>>,
) -> Summary {
    let to = session.drops.last().map_or(s.now, |d| d.at);
    let drops: Vec<&Drop> = session.drops.iter().collect();
    Summary {
        from: session.started_at,
        to,
        drops: u32::try_from(drops.len()).unwrap_or(u32::MAX),
        games: session.finished.len(),
        rate: s.forecast.map(|f| f.rate),
        estimate: session.first_forecast.as_ref().map(|f| {
            let took = (to - f.made_at).to_std().unwrap_or_default();
            Checked {
                said: f.eta,
                band: f.band,
                made_at: f.made_at,
                took,
                inside: f
                    .band
                    .is_some_and(|(low, high)| low <= took && took <= high),
            }
        }),
        value: session_value(s, &drops, s.basis),
        foils: session
            .drops
            .iter()
            .filter(|d| d.card.is_foil())
            .filter_map(|d| d.card.name().map(str::to_owned))
            .collect(),
        estimated,
        next_look,
        why: String::new(),
    }
}

/// Why nothing is left to farm, in the summary's words: every game is done,
/// or done or skipped; otherwise the farmer's own reason, `note`.
fn why_nothing(s: &Snapshot<'_>, note: &str) -> String {
    let mut left = s.library().with_drops_left().peekable();
    if left.peek().is_none() {
        "all done".to_owned()
    } else if left.all(|g| s.prefs.tier(g.app_id) == Tier::Skip) {
        "all done or skipped".to_owned()
    } else {
        note.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use market::Currency;

    use super::*;
    use crate::{tui::format, viewmodel::fixtures};

    #[test]
    fn the_time_to_finish_is_learnt_from_the_session() {
        let data = fixtures::farming_alone();
        let p = Progress::build(&data.snapshot(), None);
        let eta = p.eta.clone().unwrap();
        assert_eq!(format::eta(eta.eta), "≈ 4d 21h");
        assert_eq!(format::band(eta.band.unwrap()), "80%: 3d 13h – 6d 15h");
        assert_eq!(format::date(eta.lands, fixtures::zone()), "Sun 4 Oct");
        assert_eq!(format::rate(eta.rate), "2.1 drops an hour");
        assert_eq!(
            (eta.learnt.drops, format::duration(eta.learnt.alone)),
            (16, "7h 40m".to_owned()),
            "learnt from 16 in 7h 40m farming alone"
        );
        assert_eq!(
            format::estimate(eta.hours_term),
            "3h",
            "incl. ≈ 3h of building hours"
        );
        assert!(!eta.assumed && !eta.holds_still);
        assert!(matches!(p.activity, Activity::Farming { .. }));
        assert_eq!(p.now, fixtures::now());
    }

    #[test]
    fn the_session_and_the_library_are_counted_apart() {
        let data = fixtures::farming_alone();
        let p = Progress::build(&data.snapshot(), None);
        assert_eq!(
            p.session,
            SessionProgress {
                drops: 16,
                drops_at_start: Some(252),
                games_done: 5,
                games_at_start: Some(62),
                started_at: Some(fixtures::at(9, 14)),
            },
            "16 of 252 drops · 5 of 62 games"
        );
        assert_eq!(
            p.library,
            LibraryProgress {
                received: 183,
                total: 421,
                games_done: 5,
                games: 63
            },
            "183 of 421 drops · 5 of 63 games"
        );
        assert_eq!(
            p.to_go,
            ToGo {
                games: 57,
                drops: 236,
                set_aside: 1
            },
            "57 games · 236 drops · 1 set aside"
        );
    }

    #[test]
    fn the_values_are_this_sessions_whats_left_and_on_completion() {
        let data = fixtures::farming_alone();
        let p = Progress::build(&data.snapshot(), None);
        let v = p.values.unwrap();
        assert_eq!(format::held(&v.held), "≥ £1.45 · 3 unpriced");
        assert_eq!(format::about(v.left.value), "≈ £15.34");
        assert_eq!(format::about(v.completion.value), "≈ £16.79");
        assert!(v.completion.excl_foils, "excl. foils");
        assert_eq!(
            (v.spares.0, format::money(v.spares.1.total)),
            (2, "£0.13".to_owned()),
            "2 spares this session: £0.13"
        );
        assert_eq!(p.pause, None);
        assert_eq!(p.summary, None);
    }

    #[test]
    fn a_first_estimate_assumes_30_minutes_a_drop() {
        let data = fixtures::first_minutes();
        let p = Progress::build(&data.snapshot(), None);
        let eta = p.eta.unwrap();
        assert!(eta.assumed && eta.band.is_none());
        assert_eq!(format::eta(eta.eta), "≈ 5d 9h");
        assert_eq!(p.session.drops, 0);
        assert_eq!((p.library.received, p.library.games_done), (167, 0));
        assert_eq!(p.to_go.games, 62);
    }

    #[test]
    fn while_farming_waits_its_time_holds_still() {
        for data in [
            fixtures::waiting_for_hades(),
            fixtures::paused(),
            fixtures::sign_in_expired(),
            fixtures::reconnecting(),
        ] {
            let p = Progress::build(&data.snapshot(), None);
            assert!(p.eta.unwrap().holds_still, "of farming left");
        }
    }

    #[test]
    fn nothing_is_known_while_the_badges_are_read() {
        let data = fixtures::reading_badges();
        let p = Progress::build(&data.snapshot(), None);
        assert_eq!(p.activity, Activity::Reading);
        assert_eq!(p.eta, None);
        assert_eq!(p.library.games, 0);
        assert_eq!(p.session.drops_at_start, None);
    }

    #[test]
    fn the_pause_shows_while_it_lasts() {
        let data = fixtures::prices_paused();
        let p = Progress::build(&data.snapshot(), None);
        assert_eq!(p.pause.map(|x| x.until), Some(fixtures::at(17, 41)));
        let mut later = fixtures::prices_paused();
        later.now = fixtures::at(17, 42);
        assert_eq!(Progress::build(&later.snapshot(), None).pause, None);
    }

    #[test]
    fn nothing_left_to_farm_sums_the_session_up() {
        let data = fixtures::nothing_to_farm();
        let estimated = Money::new(1_679, Currency::GBP);
        let p = Progress::build(&data.snapshot(), Some(estimated));
        let summary = p.summary.unwrap();
        let zone = fixtures::zone();
        assert_eq!(
            (
                format::day_and_time(summary.from, zone),
                format::day_and_time(summary.to, zone)
            ),
            ("Tue 09:14".to_owned(), "Sun 17:44".to_owned())
        );
        assert_eq!((summary.drops, summary.games), (252, 62));
        assert_eq!(
            summary.rate.map(format::rate).as_deref(),
            Some("2.1 drops an hour")
        );
        assert_eq!(
            format::held(&summary.value.unwrap()),
            "≥ £17.31 · 4 unpriced"
        );
        assert_eq!(summary.foils, ["Thanatos", "The Lamb"]);
        assert_eq!(summary.estimated, Some(estimated));
        let checked = summary.estimate.unwrap();
        assert_eq!(format::duration(checked.said), "5d 6h");
        assert_eq!(format::clock(checked.made_at, summary.from, zone), "10:12");
        assert_eq!(
            format::duration(checked.took),
            "5d 8h",
            "it took 5d 8h from then"
        );
        assert_eq!(format::band(checked.band.unwrap()), "80%: 2d 18h – 10d 1h");
        assert!(checked.inside);
        assert_eq!(
            summary
                .next_look
                .map(|t| format::day_and_time(t, zone))
                .as_deref(),
            Some("Mon 01:44")
        );
        assert_eq!(p.library.received, 419);
        assert_eq!(p.to_go.games, 0);
        assert_eq!(
            summary.why, "all done or skipped",
            "Counter-Strike 2, skipped, has cards left"
        );
    }

    #[test]
    fn nothing_left_says_why_in_its_own_words() {
        let mut data = fixtures::nothing_to_farm();
        data.prefs.skipped_games.clear();
        let p = Progress::build(&data.snapshot(), None);
        assert_eq!(
            p.summary.unwrap().why,
            "every game with cards left is skipped",
            "the farmer's reason, when it isn't that everything is done or skipped"
        );
    }
}
