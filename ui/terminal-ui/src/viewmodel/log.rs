// The log and the strip (docs/design/ui.md §2.1, §5.4, §5.8): every event
// with its glyph, the market's events written with each game's name and
// times on the local clock, and what the strip shows: a flash for 4 seconds,
// a new event for as long, an alert while it lasts, else the newest event.

use chrono::{DateTime, Utc};
use farming::EventKind;
use market::MarketEventKind;

use super::{
    market::MarketNews,
    screen::{Activity, Snapshot},
};
use crate::tui::format;

/// What a log line is about, and so its glyph and colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogKind {
    /// ✓ a card dropped, or was named.
    Dropped,
    /// ▶ started playing.
    Playing,
    /// ↻ moved on before it was done.
    MovedOn,
    /// ‖ waiting: another device, or Steam's pause on prices.
    Waiting,
    /// ! a warning.
    Warning,
    /// ✕ an error.
    Error,
    /// · the rest.
    Info,
    /// · a routine look at the cards, left off the strip.
    Progress,
}

impl LogKind {
    /// The farmer's events, as the log tells them.
    pub fn of(kind: EventKind) -> Self {
        match kind {
            EventKind::Dropped | EventKind::Identified => Self::Dropped,
            EventKind::Playing => Self::Playing,
            EventKind::Switched => Self::MovedOn,
            EventKind::Warning => Self::Warning,
            EventKind::Error => Self::Error,
            EventKind::Info => Self::Info,
            EventKind::Progress => Self::Progress,
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Dropped => "✓",
            Self::Playing => "▶",
            Self::MovedOn => "↻",
            Self::Waiting => "‖",
            Self::Warning => "!",
            Self::Error => "✕",
            Self::Info | Self::Progress => "·",
        }
    }
}

/// One line of the log: when, what about, and what happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub at: DateTime<Utc>,
    pub kind: LogKind,
    pub text: String,
}

/// A market event as the log writes it: from its kind, with each game's
/// name and times on the local clock, which the market knows neither of.
/// "Prices: all 57 games looked up; the next round is at 21:21".
pub fn market_line(news: &MarketNews, s: &Snapshot<'_>) -> LogEntry {
    let at = |t: DateTime<Utc>| format::clock(t, s.now, s.zone);
    let (kind, text) = match news.event.kind {
        MarketEventKind::AllPriced { games, next_round } => {
            let games = match games {
                1 => "the 1 game".to_owned(),
                n => format!("all {n} games"),
            };
            let next = next_round
                .map_or_else(String::new, |t| format!("; the next round is at {}", at(t)));
            (
                LogKind::Progress,
                format!("Prices: {games} looked up{next}"),
            )
        }
        MarketEventKind::Paused(p) if news.again => (
            LogKind::Waiting,
            format!(
                "Steam turned down price lookups again: they wait until {}. Farming carries on.",
                at(p.until)
            ),
        ),
        MarketEventKind::Paused(p) => (
            LogKind::Waiting,
            format!(
                "Steam turned down a price lookup: lookups wait until {}. Farming carries on.",
                at(p.until)
            ),
        ),
        MarketEventKind::Resumed => (
            LogKind::Info,
            "Steam's pause on price lookups is over: they carry on.".to_owned(),
        ),
        MarketEventKind::Failed(app_id) => {
            let again = s
                .prices
                .sets
                .get(&app_id)
                .and_then(|set| set.retry_at)
                .map_or_else(|| "in 24 hours".to_owned(), |t| format!("at {}", at(t)));
            (
                LogKind::Warning,
                format!(
                    "Couldn't look up the prices of {}'s cards: they're tried again {again}.",
                    s.name(app_id)
                ),
            )
        }
        MarketEventKind::Unanswered { .. } => (LogKind::Warning, news.event.message.clone()),
    };
    LogEntry {
        at: s.now,
        kind,
        text,
    }
}

/// What the strip says about the newest drop, while its card is being found
/// out or couldn't be told.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// "· finding out which card".
    Identifying,
    /// "· couldn't tell which card".
    Unknown,
}

/// An alert: something that holds the strip while it lasts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Alert {
    /// Steam has paused price lookups: the line that told of it.
    PricesPaused(LogEntry),
    /// The connection to Steam went, at, for this reason.
    Reconnecting {
        at: Option<DateTime<Utc>>,
        why: String,
    },
    /// Steam no longer takes the saved sign-in: farming stopped then.
    Expired { at: Option<DateTime<Utc>> },
}

/// What the strip shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Strip {
    /// What a key just did, or that it didn't stick.
    Flash {
        text: String,
        failed: bool,
    },
    /// An event, and what's still being found out about it.
    Event {
        entry: LogEntry,
        detail: Option<Detail>,
    },
    Alert(Alert),
    Empty,
}

/// A flash on the strip, and whether it's still fresh (under 4 seconds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flash {
    pub text: String,
    pub failed: bool,
}

impl Strip {
    /// The strip: a flash, while fresh; then a new event, while fresh; then
    /// an alert, while it lasts; else the newest event. `newest` is the
    /// newest event that isn't a routine look, and whether it's fresh.
    /// `paused_by` is the line that told of Steam's pause on prices.
    pub fn build(
        s: &Snapshot<'_>,
        flash: Option<Flash>,
        newest: Option<(&LogEntry, bool)>,
        paused_by: Option<&LogEntry>,
    ) -> Self {
        if let Some(Flash { text, failed }) = flash {
            return Self::Flash { text, failed };
        }
        let event = |entry: &LogEntry| Self::Event {
            entry: entry.clone(),
            detail: detail(s, entry),
        };
        if let Some((entry, true)) = newest {
            return event(entry);
        }
        if let Some(alert) = alert(s, paused_by) {
            return Self::Alert(alert);
        }
        newest.map_or(Self::Empty, |(entry, _)| event(entry))
    }
}

/// The alert in force, if any: an expired sign-in first, then a lost
/// connection, then Steam's pause on prices.
fn alert(s: &Snapshot<'_>, paused_by: Option<&LogEntry>) -> Option<Alert> {
    match Activity::of(s) {
        Activity::Expired { stopped_at } => return Some(Alert::Expired { at: stopped_at }),
        Activity::Reconnecting { why, .. } => {
            let at = s
                .session()
                .and_then(|ss| ss.stretches.last())
                .and_then(|st| st.to);
            return Some(Alert::Reconnecting { at, why });
        }
        _ => {}
    }
    let paused = s.pause.is_some_and(|p| p.until > s.now);
    paused_by
        .filter(|_| paused)
        .map(|line| Alert::PricesPaused(line.clone()))
}

/// What's still being found out about the drop an event tells of: the
/// session's newest drop, while it's being identified or couldn't be, when
/// the event is about its game.
fn detail(s: &Snapshot<'_>, entry: &LogEntry) -> Option<Detail> {
    let drop = s.drops().last()?;
    let about = format!(" for {} ", s.name(drop.app_id));
    if entry.kind != LogKind::Dropped || !format!("{} ", entry.text).contains(&about) {
        return None;
    }
    match drop.card {
        farming::DropCard::Identifying => Some(Detail::Identifying),
        farming::DropCard::Unknown => Some(Detail::Unknown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::TimeDelta;
    use market::{MarketEvent, MarketPause};

    use super::*;
    use crate::viewmodel::fixtures;

    fn news(kind: MarketEventKind, again: bool) -> MarketNews {
        MarketNews {
            event: MarketEvent {
                kind,
                message:
                    "Couldn't ask the market for prices: no sign-in. Asking again in a minute."
                        .into(),
            },
            again,
        }
    }

    #[test]
    fn the_farmers_events_keep_their_glyphs() {
        let glyphs: Vec<&str> = [
            EventKind::Dropped,
            EventKind::Identified,
            EventKind::Playing,
            EventKind::Switched,
            EventKind::Warning,
            EventKind::Error,
            EventKind::Info,
            EventKind::Progress,
        ]
        .into_iter()
        .map(|k| LogKind::of(k).glyph())
        .collect();
        assert_eq!(glyphs, ["✓", "✓", "▶", "↻", "!", "✕", "·", "·"]);
        assert_eq!(LogKind::Waiting.glyph(), "‖");
    }

    #[test]
    fn the_markets_lines_have_names_and_the_local_clock() {
        let data = fixtures::farming_alone();
        let s = data.snapshot();
        let line = |kind, again| market_line(&news(kind, again), &s);

        let all = line(
            MarketEventKind::AllPriced {
                games: 57,
                next_round: Some(fixtures::at(21, 21)),
            },
            false,
        );
        assert_eq!(
            all.text,
            "Prices: all 57 games looked up; the next round is at 21:21"
        );
        assert_eq!(
            (all.kind, all.at),
            (LogKind::Progress, fixtures::now()),
            "routine"
        );
        assert_eq!(
            line(
                MarketEventKind::AllPriced {
                    games: 1,
                    next_round: None
                },
                false
            )
            .text,
            "Prices: the 1 game looked up"
        );

        let pause = MarketPause {
            until: fixtures::at(17, 41),
            step: Duration::from_secs(3600),
        };
        let again = line(MarketEventKind::Paused(pause), true);
        assert_eq!(
            again.text,
            "Steam turned down price lookups again: they wait until 17:41. Farming carries on."
        );
        assert_eq!(again.kind, LogKind::Waiting);
        assert_eq!(
            line(MarketEventKind::Paused(pause), false).text,
            "Steam turned down a price lookup: lookups wait until 17:41. Farming carries on."
        );
        assert_eq!(
            line(MarketEventKind::Resumed, false).text,
            "Steam's pause on price lookups is over: they carry on."
        );
        assert_eq!(
            line(MarketEventKind::Failed(fixtures::HEAVY_RAIN), false).text,
            "Couldn't look up the prices of Heavy Rain's cards: they're tried again in 24 hours."
        );
        let unanswered = line(
            MarketEventKind::Unanswered {
                retry_at: fixtures::now(),
            },
            false,
        );
        assert_eq!(unanswered.kind, LogKind::Warning);
        assert!(unanswered.text.starts_with("Couldn't ask the market"));
    }

    #[test]
    fn a_failed_lookup_says_when_its_tried_again() {
        let mut data = fixtures::farming_alone();
        if let Some(set) = data.prices.sets.get_mut(&fixtures::LIMBO) {
            set.retry_at = Some(fixtures::now() + TimeDelta::days(1));
        }
        let line = market_line(
            &news(MarketEventKind::Failed(fixtures::LIMBO), false),
            &data.snapshot(),
        );
        assert_eq!(
            line.text,
            "Couldn't look up the prices of LIMBO's cards: they're tried again at Wed 17:31."
        );
    }

    fn dropped() -> LogEntry {
        LogEntry {
            at: fixtures::at(17, 23),
            kind: LogKind::Dropped,
            text: "A card dropped for Heavy Rain — 1 to go".into(),
        }
    }

    #[test]
    fn the_strip_names_what_a_drop_is_still_waiting_for() {
        let data = fixtures::farming_alone();
        let s = data.snapshot();
        let entry = dropped();
        assert_eq!(
            Strip::build(&s, None, Some((&entry, false)), None),
            Strip::Event {
                entry: entry.clone(),
                detail: Some(Detail::Identifying)
            },
            "17:23 ✓ A card dropped for Heavy Rain — 1 to go · finding out which card"
        );
        let flash = Flash {
            text: "Opened Heavy Rain's card page.".into(),
            failed: false,
        };
        assert_eq!(
            Strip::build(&s, Some(flash.clone()), Some((&entry, true)), None),
            Strip::Flash {
                text: flash.text,
                failed: false
            }
        );
        assert_eq!(Strip::build(&s, None, None, None), Strip::Empty);
    }

    #[test]
    fn an_alert_holds_the_strip_but_a_new_event_takes_it_for_a_moment() {
        let data = fixtures::prices_paused();
        let s = data.snapshot();
        let paused = LogEntry {
            at: fixtures::at(16, 41),
            kind: LogKind::Waiting,
            text:
                "Steam turned down price lookups again: they wait until 17:41. Farming carries on."
                    .into(),
        };
        let entry = dropped();
        assert_eq!(
            Strip::build(&s, None, Some((&entry, false)), Some(&paused)),
            Strip::Alert(Alert::PricesPaused(paused.clone()))
        );
        assert!(matches!(
            Strip::build(&s, None, Some((&entry, true)), Some(&paused)),
            Strip::Event { .. }
        ));
        let farming = fixtures::farming_alone();
        assert!(
            matches!(
                Strip::build(
                    &farming.snapshot(),
                    None,
                    Some((&entry, false)),
                    Some(&paused)
                ),
                Strip::Event { .. }
            ),
            "no pause, no alert"
        );
    }

    #[test]
    fn a_lost_connection_or_an_expired_sign_in_holds_the_strip() {
        let data = fixtures::reconnecting();
        assert_eq!(
            Strip::build(&data.snapshot(), None, None, None),
            Strip::Alert(Alert::Reconnecting {
                at: Some(fixtures::at(17, 30)),
                why: "the connection was reset".into()
            })
        );
        let data = fixtures::sign_in_expired();
        assert_eq!(
            Strip::build(&data.snapshot(), None, None, None),
            Strip::Alert(Alert::Expired {
                at: Some(fixtures::now())
            })
        );
    }
}
