// This session's cards (docs/design/ui.md §2.1, mockups a and j): every copy
// that dropped on its own row with its price and a running total that turns
// "≥" at the first gap; the cards by game with their spares; the session on
// each basis; the pace; why any card isn't priced; and the day as a track.

use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use farming::{Drop, Mode};
use market::{Basis, Held, Money};

use super::{
    now::Told,
    screen::{Cell, Snapshot, held_card, session_value},
};

/// The haul: the dashboard's panel and the pop-up h opens.
#[derive(Debug, Clone, PartialEq)]
pub struct Haul {
    /// Every drop, oldest first.
    pub rows: Vec<HaulRow>,
    pub by_game: Vec<GameHaul>,
    /// Every card on the basis.
    pub total: Option<Held>,
    pub spares: u32,
    pub foils: u32,
    /// The session on each basis: list, net, instant.
    pub at_each: Option<[(Basis, Held); 3]>,
    pub pace: Pace,
    /// Why each card that isn't priced isn't, oldest first.
    pub unpriced: Vec<Unpriced>,
    pub track: Track,
    pub basis: Basis,
    pub now: DateTime<Utc>,
    pub zone: FixedOffset,
}

/// One copy that dropped.
#[derive(Debug, Clone, PartialEq)]
pub struct HaulRow {
    pub at: DateTime<Utc>,
    pub app_id: u32,
    pub game: String,
    pub card: Told,
    /// Which copy it made the account hold: 2 or more is a spare; `None`
    /// while that isn't known.
    pub copy: Option<u32>,
    /// Its price on the basis; `None` while the card isn't known.
    pub price: Option<Cell>,
    /// The running total up to and including it.
    pub total: Option<Money>,
    /// Some card up to here isn't priced, so the total is at least.
    pub at_least: bool,
}

/// What a game's state is, for the haul's by-game table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameState {
    /// ✓ every card has dropped.
    Done,
    /// ▶ farmed alone now.
    Farming,
    Other,
}

/// A game's cards this session.
#[derive(Debug, Clone, PartialEq)]
pub struct GameHaul {
    pub app_id: u32,
    pub name: String,
    pub state: GameState,
    pub cards: u32,
    pub spares: u32,
    pub value: Option<Held>,
}

/// How fast cards come, and what they're worth an hour.
#[derive(Debug, Clone, PartialEq)]
pub struct Pace {
    /// Drops an hour, farming alone, as the forecast learnt it.
    pub rate: Option<f64>,
    /// This session's value an hour so far, at least, on the basis.
    pub per_hour: Option<Money>,
    /// The session's most valuable card.
    pub best: Option<Best>,
}

/// The session's most valuable card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Best {
    pub name: String,
    pub game: String,
    pub foil: bool,
    pub value: Money,
}

/// Why a card this session isn't priced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Why {
    /// Its price is on its way: "… on its way".
    Pending,
    /// Nobody is selling it: "no market".
    NoMarket,
    /// Its lookup failed: tried again later.
    Failed,
    /// Priced in another currency, never converted.
    Foreign,
    /// Which card it was is being found out: "not known yet".
    Identifying,
    /// Nothing could tell which card it was.
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unpriced {
    pub at: DateTime<Utc>,
    pub game: String,
    /// The card's name, when it's known.
    pub card: Option<String>,
    pub why: Why,
}

/// The session as a line: a mark for each drop, where one game handed over
/// to the next, and each game's name under its stretch. A session longer
/// than a day draws its days and foils instead.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    /// Each drop: when, and whether a foil.
    pub marks: Vec<(DateTime<Utc>, bool)>,
    /// Each game farmed in turn, and from when to when: a stretch lasts
    /// until the next game's begins. Games played together aren't named.
    pub games: Vec<(String, DateTime<Utc>, DateTime<Utc>)>,
    /// Longer than a day: days and foils, not every drop.
    pub by_day: bool,
}

impl Haul {
    pub fn build(s: &Snapshot<'_>) -> Self {
        let drops = s.drops();
        let wallet = s.wallet;
        let mut total = wallet.map(|w| Money::zero(w.currency));
        let mut gap = false;
        let rows: Vec<HaulRow> = drops
            .iter()
            .map(|d| {
                let price = held_card(d).map(|card| {
                    Cell::of(
                        &s.prices.price(&card, s.basis),
                        s.basis,
                        wallet.as_ref(),
                        s.now,
                    )
                });
                match price.and_then(|p| p.value()) {
                    Some(v) => total = total.and_then(|t| t.checked_add(v)),
                    None if price != Some(Cell::NotMarketable) => gap = true,
                    None => {}
                }
                HaulRow {
                    at: d.at,
                    app_id: d.app_id,
                    game: s.name(d.app_id),
                    card: Told::of(&d.card),
                    copy: d.copy,
                    price,
                    total,
                    at_least: gap,
                }
            })
            .collect();
        let all: Vec<&Drop> = drops.iter().collect();
        let held = session_value(s, &all, s.basis);
        let at_each = (|| {
            Some([
                (Basis::List, session_value(s, &all, Basis::List)?),
                (Basis::Net, session_value(s, &all, Basis::Net)?),
                (Basis::Instant, session_value(s, &all, Basis::Instant)?),
            ])
        })();
        let hours = s.session().map_or(0.0, |ss| {
            (s.now - ss.started_at).num_seconds() as f64 / 3600.0
        });
        let per_hour = held.filter(|_| hours > 0.0).map(|h| {
            Money::new(
                (h.total.minor as f64 / hours).floor() as i64,
                h.total.currency,
            )
        });
        let best = rows
            .iter()
            .filter_map(|r| {
                let value = r.price?.value()?;
                let Told::Named { name, foil } = &r.card else {
                    return None;
                };
                Some(Best {
                    name: name.clone(),
                    game: r.game.clone(),
                    foil: *foil,
                    value,
                })
            })
            .max_by_key(|b| b.value.minor);
        Self {
            by_game: by_game(s, &all),
            total: held,
            spares: u32::try_from(drops.iter().filter(|d| d.is_spare()).count()).unwrap_or(0),
            foils: u32::try_from(drops.iter().filter(|d| d.card.is_foil()).count()).unwrap_or(0),
            at_each,
            pace: Pace {
                rate: s.forecast.map(|f| f.rate),
                per_hour,
                best,
            },
            unpriced: unpriced(&rows),
            track: track(s),
            rows,
            basis: s.basis,
            now: s.now,
            zone: s.zone,
        }
    }
}

fn by_game(s: &Snapshot<'_>, drops: &[&Drop]) -> Vec<GameHaul> {
    let mut games: Vec<u32> = Vec::new();
    for d in drops {
        if !games.contains(&d.app_id) {
            games.push(d.app_id);
        }
    }
    let farming = s.farming_alone().map(|g| g.app_id);
    games
        .into_iter()
        .map(|app_id| {
            let mine: Vec<&Drop> = drops
                .iter()
                .copied()
                .filter(|d| d.app_id == app_id)
                .collect();
            let done = s
                .library()
                .game(app_id)
                .is_some_and(|g| !g.has_drops_left());
            GameHaul {
                app_id,
                name: s.name(app_id),
                state: if done {
                    GameState::Done
                } else if farming == Some(app_id) {
                    GameState::Farming
                } else {
                    GameState::Other
                },
                cards: u32::try_from(mine.len()).unwrap_or(u32::MAX),
                spares: u32::try_from(mine.iter().filter(|d| d.is_spare()).count())
                    .unwrap_or(u32::MAX),
                value: session_value(s, &mine, s.basis),
            }
        })
        .collect()
}

fn unpriced(rows: &[HaulRow]) -> Vec<Unpriced> {
    rows.iter()
        .filter_map(|r| {
            let why = match (&r.card, r.price) {
                (Told::Identifying, _) => Why::Identifying,
                (Told::Unknown, _) => Why::Unknown,
                (_, Some(Cell::Pending)) => Why::Pending,
                (_, Some(Cell::NoMarket)) => Why::NoMarket,
                (_, Some(Cell::Failed)) => Why::Failed,
                (_, Some(Cell::Foreign(_))) => Why::Foreign,
                _ => return None,
            };
            Some(Unpriced {
                at: r.at,
                game: r.game.clone(),
                card: match &r.card {
                    Told::Named { name, .. } => Some(name.clone()),
                    Told::Identifying | Told::Unknown => None,
                },
                why,
            })
        })
        .collect()
}

fn track(s: &Snapshot<'_>) -> Track {
    let session = s.session();
    let from = session.map_or(s.now, |ss| ss.started_at);
    let marks = s.drops().iter().map(|d| (d.at, d.card.is_foil())).collect();
    // Each game farmed alone in turn, a stretch running on to the next
    // game's: a pause or a moment between games doesn't split it.
    let mut games: Vec<(u32, DateTime<Utc>)> = Vec::new();
    for stretch in session.map_or(&[][..], |ss| ss.stretches.as_slice()) {
        let game = match (stretch.mode, stretch.app_ids.as_slice()) {
            (Mode::Cards, [id]) => *id,
            _ => 0,
        };
        if games.last().is_none_or(|(last, _)| *last != game) {
            games.push((game, stretch.from));
        }
    }
    let spans = games
        .iter()
        .enumerate()
        .filter(|(_, (id, _))| *id != 0)
        .map(|(i, &(id, start))| {
            let end = games.get(i + 1).map_or(s.now, |next| next.1);
            (s.name(id), start, end)
        })
        .collect();
    Track {
        from,
        to: s.now,
        marks,
        games: spans,
        by_day: (s.now - from).num_seconds() > 24 * 3600,
    }
}

impl Track {
    /// How long it spans.
    pub fn length(&self) -> Duration {
        (self.to - self.from).to_std().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tui::format, viewmodel::fixtures};

    fn row_text(r: &HaulRow) -> (String, String, String, String) {
        let zone = fixtures::zone();
        let card = match &r.card {
            Told::Named { name, foil } => {
                let star = if *foil { "★ " } else { "" };
                let copy = match r.copy {
                    Some(2) => ", 2nd copy",
                    _ => "",
                };
                format!("{star}{name}{copy}")
            }
            Told::Identifying => "identifying".into(),
            Told::Unknown => "unknown".into(),
        };
        let price = match r.price {
            Some(Cell::Value { value, .. }) => format::money(value),
            Some(Cell::Pending) | None => "…".into(),
            Some(Cell::NoMarket) => "no market".into(),
            Some(other) => format!("{other:?}"),
        };
        let total = r
            .total
            .map(|t| format!("{}{}", if r.at_least { "≥ " } else { "" }, format::money(t)))
            .unwrap_or_default();
        (
            format::clock(r.at, fixtures::now(), zone),
            card,
            price,
            total,
        )
    }

    #[test]
    fn every_copy_has_its_row_and_the_total_turns_at_least_at_the_first_gap() {
        let data = fixtures::farming_alone();
        let haul = Haul::build(&data.snapshot());
        let rows: Vec<(String, String, String, String)> = haul.rows.iter().map(row_text).collect();
        let want = [
            ("09:44", "Hornet", "£0.09", "£0.09"),
            ("10:12", "Zote", "£0.07", "£0.16"),
            ("10:41", "The Knight", "£0.11", "£0.27"),
            ("11:15", "Leshy", "£0.06", "£0.33"),
            ("11:43", "Stoat", "£0.05", "£0.38"),
            ("12:20", "Stinkbug", "£0.05", "£0.43"),
            ("12:58", "Zagreus", "£0.08", "£0.51"),
            ("13:30", "Zagreus, 2nd copy", "£0.08", "£0.59"),
            ("14:02", "★ Thanatos", "£0.62", "£1.21"),
            ("14:35", "Nyx", "£0.09", "£1.30"),
            ("15:09", "Madeline", "…", "≥ £1.30"),
            ("15:41", "Badeline", "£0.06", "≥ £1.36"),
            ("16:15", "The Boy", "£0.04", "≥ £1.40"),
            ("16:48", "The Fruit", "no market", "≥ £1.40"),
            ("17:05", "Madison, 2nd copy", "£0.05", "≥ £1.45"),
            ("17:23", "identifying", "…", "≥ £1.45"),
        ];
        assert_eq!(rows.len(), want.len());
        for (row, want) in rows.iter().zip(want) {
            assert_eq!(
                (
                    row.0.as_str(),
                    row.1.as_str(),
                    row.2.as_str(),
                    row.3.as_str()
                ),
                want
            );
        }
        assert_eq!(haul.rows[14].game, "Heavy Rain");
        assert_eq!(haul.rows[14].app_id, fixtures::HEAVY_RAIN);
    }

    #[test]
    fn the_cards_by_game_with_their_spares() {
        let data = fixtures::farming_alone();
        let haul = Haul::build(&data.snapshot());
        let by: Vec<(GameState, &str, u32, u32, String)> = haul
            .by_game
            .iter()
            .map(|g| {
                (
                    g.state,
                    g.name.as_str(),
                    g.cards,
                    g.spares,
                    format::at_least(&g.value.unwrap()),
                )
            })
            .collect();
        assert_eq!(
            by,
            [
                (GameState::Done, "Hollow Knight", 3, 0, "£0.27".to_owned()),
                (GameState::Done, "Inscryption", 3, 0, "£0.16".to_owned()),
                (GameState::Done, "Hades", 4, 1, "£0.87".to_owned()),
                (GameState::Done, "Celeste", 2, 0, "≥ £0.06".to_owned()),
                (GameState::Done, "Gorogoa", 2, 0, "≥ £0.04".to_owned()),
                (GameState::Farming, "Heavy Rain", 2, 1, "≥ £0.05".to_owned()),
            ]
        );
        assert_eq!(haul.by_game[5].app_id, fixtures::HEAVY_RAIN);
        assert_eq!((haul.spares, haul.foils), (2, 1));
        assert_eq!(format::held(&haul.total.unwrap()), "≥ £1.45 · 3 unpriced");
    }

    #[test]
    fn the_session_at_each_basis() {
        let data = fixtures::farming_alone();
        let haul = Haul::build(&data.snapshot());
        let at_each: Vec<(Basis, String)> = haul
            .at_each
            .unwrap()
            .iter()
            .map(|(b, h)| (*b, format::at_least(h)))
            .collect();
        assert_eq!(
            at_each,
            [
                (Basis::List, "≥ £1.45".to_owned()),
                (Basis::Net, "≥ £1.14".to_owned()),
                (Basis::Instant, "≥ £0.74".to_owned())
            ]
        );
        assert_eq!(haul.basis, Basis::List);
        assert_eq!(haul.now, fixtures::now());
    }

    #[test]
    fn the_pace_and_the_best_card() {
        let data = fixtures::farming_alone();
        let pace = Haul::build(&data.snapshot()).pace;
        assert_eq!(
            pace.rate.map(format::rate).as_deref(),
            Some("2.1 drops an hour")
        );
        assert_eq!(
            pace.per_hour.map(format::money).as_deref(),
            Some("£0.17"),
            "≥ £0.17 an hour so far"
        );
        let best = pace.best.unwrap();
        assert_eq!(
            (
                best.name.as_str(),
                best.game.as_str(),
                best.foil,
                format::money(best.value)
            ),
            ("Thanatos", "Hades", true, "£0.62".to_owned()),
            "best: ★ Thanatos (Hades), £0.62"
        );
    }

    #[test]
    fn why_each_card_isnt_priced() {
        let data = fixtures::farming_alone();
        let haul = Haul::build(&data.snapshot());
        let why: Vec<(Option<&str>, &str, &Why)> = haul
            .unpriced
            .iter()
            .map(|u| (u.card.as_deref(), u.game.as_str(), &u.why))
            .collect();
        assert_eq!(
            why,
            [
                (Some("Madeline"), "Celeste", &Why::Pending),
                (Some("The Fruit"), "Gorogoa", &Why::NoMarket),
                (None, "Heavy Rain", &Why::Identifying),
            ],
            "Madeline waits for its price, nobody is selling The Fruit, and the 17:23 card is \
             still being identified"
        );
        assert_eq!(haul.unpriced[2].at, fixtures::at(17, 23));
    }

    #[test]
    fn the_track_draws_the_day() {
        let data = fixtures::farming_alone();
        let track = Haul::build(&data.snapshot()).track;
        assert_eq!(
            (track.from, track.to),
            (fixtures::at(9, 14), fixtures::now())
        );
        assert_eq!(format::duration(track.length()), "8h 17m");
        assert_eq!(track.marks.len(), 16);
        assert_eq!(track.marks.iter().filter(|m| m.1).count(), 1, "★ the foil");
        let games: Vec<(&str, String, String)> = track
            .games
            .iter()
            .map(|(name, a, b)| {
                let zone = fixtures::zone();
                (
                    name.as_str(),
                    format::clock(*a, track.to, zone),
                    format::clock(*b, track.to, zone),
                )
            })
            .collect();
        let want = [
            ("Hollow Knight", "09:14", "10:46"),
            ("Inscryption", "10:46", "12:25"),
            ("Hades", "12:25", "14:40"),
            ("Celeste", "14:40", "15:46"),
            ("Gorogoa", "15:46", "16:53"),
            ("Heavy Rain", "16:53", "17:31"),
        ];
        assert_eq!(games.len(), want.len(), "the pause didn't split Hades");
        for (game, want) in games.iter().zip(want) {
            assert_eq!((game.0, game.1.as_str(), game.2.as_str()), want);
        }
        assert!(!track.by_day);
        let week = fixtures::nothing_to_farm();
        assert!(Haul::build(&week.snapshot()).track.by_day, "days and foils");
    }

    #[test]
    fn a_group_building_hours_isnt_named_on_the_track() {
        let data = fixtures::building_hours();
        let track = Haul::build(&data.snapshot()).track;
        let last = track.games.last().unwrap();
        assert_eq!(last.0, "Heavy Rain");
        assert_eq!(last.2, fixtures::at(17, 24), "until the group began");
    }
}
