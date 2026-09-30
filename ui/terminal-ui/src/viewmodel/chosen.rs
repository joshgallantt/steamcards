// The chosen game: everything about the game under the cursor, for its panel
// beside the queue and its details (docs/design/ui.md §2.1, mockups a and i):
// what's happening to it and why, its drops and this session's, its value,
// the whole set with a count per card and its prices, how many cards short
// of a badge, when it was priced, and its farm priority.

use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use farming::{Mode, SetAside, hours_to_go};
use library::Game;
use market::{Basis, Held, Money, Price, expected_per_drop, value_left, value_of};
use preferences::Tier;

use super::{
    now::{Pips, Told},
    screen::{Activity, Cell, Snapshot, session_value},
};

/// What's happening to the chosen game.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Doing {
    /// ▶ played on its own, its cards dropping.
    Farming,
    /// ▷ played with others to build its hours.
    BuildingHours { others: usize },
    /// Farmed next, once another device stops playing.
    Waiting,
    /// First in line while farming is stopped.
    NextUp,
    /// In line, farmed in turn.
    Queued,
    /// Put behind the others after 10 hours without a drop.
    SetAside(SetAside),
    /// Never farmed: the user skipped it.
    Skipped,
    /// Not farmed: "only priority" is on, and it isn't one.
    NotFarmed,
    /// Every card has dropped, this session or before.
    Done { at: Option<DateTime<Utc>> },
}

/// The chosen game, as its panel and its details show it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChosenGame {
    pub app_id: u32,
    pub name: String,
    pub badge_level: u8,
    pub doing: Doing,
    pub tier: Tier,
    pub hours: f64,
    /// Hours it still needs before its cards can drop.
    pub hours_to_go: f64,
    pub pips: Pips,
    pub received: u32,
    pub total: u32,
    pub remaining: u32,
    /// What a drop is likely worth, and all its drops left.
    pub per_drop: Option<Cell>,
    pub left: Option<Cell>,
    /// When its last card should drop, from now.
    pub done_in: Option<Duration>,
    /// How often its card page is looked at, while it's farmed alone.
    pub look_every: Option<Duration>,
    /// Its set; `None` until its card page is read.
    pub set: Option<TheSet>,
    /// Its set's price ranges, normal and foil: what shows while the set
    /// isn't read.
    pub normal_range: Option<(Money, Money)>,
    pub foil_range: Option<(Money, Money)>,
    /// When its prices were looked up, and their age once over 6 hours.
    pub priced_at: Option<DateTime<Utc>>,
    pub stale: Option<Duration>,
    /// This session's drops of it, oldest first.
    pub this_session: Vec<SessionCard>,
    /// This session's cards of it, on the basis.
    pub value_this_session: Option<Held>,
    pub basis: Basis,
    pub now: DateTime<Utc>,
    pub zone: FixedOffset,
}

/// One of this session's drops of the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCard {
    pub at: DateTime<Utc>,
    pub card: Told,
    pub copy: Option<u32>,
}

/// A game's set, as a badge sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct TheSet {
    pub cards: Vec<SetCard>,
    /// Distinct cards held.
    pub have: usize,
    pub spares: u32,
    /// Cards with none held: what a badge is short of.
    pub missing: usize,
    /// What they'd cost to buy, at list prices; `None` while any isn't
    /// priced.
    pub missing_cost: Option<Money>,
}

/// A card of the set: how many are held, what it sells for, normal and foil
/// at list prices, and to offers now, and the copies that dropped today.
#[derive(Debug, Clone, PartialEq)]
pub struct SetCard {
    pub name: String,
    pub owned: u32,
    pub normal: Cell,
    pub foil: Cell,
    /// What selling it now pays: its best offer, after fees. Known only once
    /// its order book is looked up, which the details ask for.
    pub sell_now: Cell,
    /// When this session's copies of it dropped.
    pub today: Vec<DateTime<Utc>>,
}

impl ChosenGame {
    /// The chosen game; `None` when it isn't in the library.
    pub fn build(s: &Snapshot<'_>, app_id: u32) -> Option<Self> {
        let g = s.library().game(app_id)?;
        let activity = Activity::of(s);
        let drops: Vec<_> = s.drops().iter().filter(|d| d.app_id == app_id).collect();
        let set_prices = s.prices.sets.get(&app_id);
        let range = |foil: bool| {
            let (set, wallet) = (set_prices?, s.wallet?);
            let border = if foil { &set.foil } else { &set.normal };
            let values: Vec<Money> = border
                .iter()
                .filter_map(|c| value_of(&c.price, s.basis.still_to_drop(), &wallet))
                .collect();
            Some((
                *values.iter().min_by_key(|m| m.minor)?,
                *values.iter().max_by_key(|m| m.minor)?,
            ))
        };
        let quote = set_prices.and_then(|set| {
            set.normal
                .iter()
                .chain(&set.foil)
                .find_map(|c| match &c.price {
                    Price::Known(q) => Some(q.clone()),
                    _ => None,
                })
        });
        let (per_drop, left) = values(s, g);
        Some(Self {
            app_id,
            name: g.name.clone(),
            badge_level: g.badge_level,
            doing: doing(s, &activity, g),
            tier: s.prefs.tier(app_id),
            hours: g.hours,
            hours_to_go: hours_to_go(g),
            pips: Pips::of(g, s.drops()),
            received: g.drops.received,
            total: g.drops.total(),
            remaining: g.drops.remaining,
            per_drop,
            left,
            done_in: s.done_in(app_id),
            look_every: s
                .status
                .and_then(|st| st.look_every)
                .filter(|_| s.farming_alone().is_some_and(|f| f.app_id == app_id)),
            set: (!g.cards.is_empty()).then(|| the_set(s, g, &drops)),
            normal_range: range(false),
            foil_range: range(true),
            priced_at: set_prices.map(|set| set.fetched_at),
            stale: quote
                .as_ref()
                .filter(|q| q.is_stale(s.now))
                .map(|q| q.age(s.now)),
            this_session: drops
                .iter()
                .map(|d| SessionCard {
                    at: d.at,
                    card: Told::of(&d.card),
                    copy: d.copy,
                })
                .collect(),
            value_this_session: (!drops.is_empty())
                .then(|| session_value(s, &drops, s.basis))
                .flatten(),
            basis: s.basis,
            now: s.now,
            zone: s.zone,
        })
    }

    /// The hashes of its cards on the market, for their order books: what
    /// sell-now prices need.
    pub fn market_hash_names(s: &Snapshot<'_>, app_id: u32) -> Vec<String> {
        s.prices.sets.get(&app_id).map_or_else(Vec::new, |set| {
            set.normal
                .iter()
                .map(|c| c.market_hash_name.clone())
                .collect()
        })
    }
}

fn values(s: &Snapshot<'_>, g: &Game) -> (Option<Cell>, Option<Cell>) {
    if !g.has_drops_left() {
        return (None, None);
    }
    let (Some(set), Some(wallet)) = (s.prices.sets.get(&g.app_id), s.wallet) else {
        return (Some(Cell::Pending), Some(Cell::Pending));
    };
    match expected_per_drop(set, s.basis, &wallet) {
        Some(value) => (
            Some(Cell::Value { value, stale: None }),
            Some(Cell::Value {
                value: value_left(s.library(), &[g.app_id], s.prices, s.basis, &wallet).value,
                stale: None,
            }),
        ),
        None if set.retry_at.is_some() => (Some(Cell::Failed), Some(Cell::Failed)),
        None => (Some(Cell::NoMarket), Some(Cell::NoMarket)),
    }
}

fn doing(s: &Snapshot<'_>, activity: &Activity, g: &Game) -> Doing {
    let playing = s.playing();
    let first = s.order().first() == Some(&g.app_id);
    let set_aside = s
        .status
        .and_then(|st| st.set_aside.iter().find(|a| a.app_id == g.app_id).copied());
    if !g.has_drops_left() {
        let at = s
            .session()
            .and_then(|ss| ss.finished.iter().find(|f| f.app_id == g.app_id))
            .map(|f| f.at);
        return Doing::Done { at };
    }
    match s.prefs.tier(g.app_id) {
        Tier::Skip => return Doing::Skipped,
        _ if !s.prefs.wants(g.app_id) => return Doing::NotFarmed,
        _ => {}
    }
    match s.mode() {
        Some(Mode::Cards) if playing.first() == Some(&g.app_id) => return Doing::Farming,
        Some(Mode::Hours) if playing.contains(&g.app_id) => {
            return Doing::BuildingHours {
                others: playing.len() - 1,
            };
        }
        _ => {}
    }
    match (activity, set_aside) {
        (Activity::Waiting { .. }, _) if first => Doing::Waiting,
        (a, _) if first && a.holds_still() => Doing::NextUp,
        (_, Some(aside)) => Doing::SetAside(aside),
        _ => Doing::Queued,
    }
}

fn the_set(s: &Snapshot<'_>, g: &Game, drops: &[&farming::Drop]) -> TheSet {
    let set = s.prices.sets.get(&g.app_id);
    let wallet = s.wallet;
    let cell = |name: &str, foil: bool, basis: Basis| match set {
        Some(set) => Cell::of(&set.price(name, foil), basis, wallet.as_ref(), s.now),
        None => Cell::Pending,
    };
    let sell_now = |name: &str| {
        let hash = set
            .and_then(|set| set.card(name, false))
            .map(|c| &c.market_hash_name);
        match hash.and_then(|h| s.prices.offers.get(h)) {
            Some(offers) => Cell::of(&offers.price, Basis::Instant, wallet.as_ref(), s.now),
            None => Cell::Pending,
        }
    };
    let cards: Vec<SetCard> = g
        .cards
        .iter()
        .map(|c| SetCard {
            name: c.name.clone(),
            owned: c.owned,
            normal: cell(&c.name, false, Basis::List),
            foil: cell(&c.name, true, Basis::List),
            sell_now: sell_now(&c.name),
            today: drops
                .iter()
                .filter(|d| {
                    matches!(Told::of(&d.card), Told::Named { ref name, foil: false } if *name == c.name)
                })
                .map(|d| d.at)
                .collect(),
        })
        .collect();
    let missing: Vec<&SetCard> = cards.iter().filter(|c| c.owned == 0).collect();
    let missing_cost = wallet.and_then(|w| {
        missing.iter().try_fold(Money::zero(w.currency), |sum, c| {
            sum.checked_add(c.normal.value()?)
        })
    });
    TheSet {
        have: g.cards_collected(),
        spares: g.spares(),
        missing: missing.len(),
        missing_cost,
        cards,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{tui::format, viewmodel::fixtures};

    fn money(c: Cell) -> String {
        c.value().map_or_else(|| format!("{c:?}"), format::money)
    }

    #[test]
    fn heavy_rain_its_set_and_its_prices() {
        let data = fixtures::farming_alone();
        let g = ChosenGame::build(&data.snapshot(), fixtures::HEAVY_RAIN).unwrap();
        assert_eq!(
            (g.app_id, g.name.as_str(), g.badge_level),
            (960_910, "Heavy Rain", 0)
        );
        assert_eq!(g.doing, Doing::Farming, "▶ farming now");
        assert_eq!(format::hours(g.hours), "4.0h");
        assert_eq!(g.hours_to_go, 0.0);
        assert_eq!(
            (g.received, g.total, g.remaining),
            (3, 4, 1),
            "3 of 4 drops · 1 to go"
        );
        assert_eq!(g.pips.today, 2);
        assert_eq!(
            g.per_drop.map(money).as_deref(),
            Some("£0.05"),
            "≈ £0.05 a drop"
        );
        assert_eq!(g.left.map(money).as_deref(), Some("£0.05"));
        assert_eq!(format::estimate(g.done_in.unwrap()), "25m");
        assert_eq!(g.look_every, Some(Duration::from_secs(300)));
        assert_eq!(g.tier, Tier::Indifferent);

        let set = g.set.unwrap();
        assert_eq!(
            format::the_set(set.have, set.cards.len(), set.spares),
            "2 of 5 cards · 1 spare"
        );
        let rows: Vec<(String, String, String, String, String)> = set
            .cards
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    format::count(c.owned),
                    money(c.normal),
                    money(c.foil),
                    money(c.sell_now),
                )
            })
            .collect();
        let want = [
            ("Ethan", "—", "£0.05", "£0.42", "£0.01"),
            ("Carter", "—", "£0.04", "£0.35", "£0.01"),
            ("Madison", "×2", "£0.05", "£0.60", "£0.02"),
            ("Norman", "—", "£0.06", "£0.51", "£0.02"),
            ("Scott", "×1", "£0.04", "£0.38", "£0.01"),
        ];
        for (row, want) in rows.iter().zip(want) {
            assert_eq!(
                (
                    row.0.as_str(),
                    row.1.as_str(),
                    row.2.as_str(),
                    row.3.as_str(),
                    row.4.as_str()
                ),
                want
            );
        }
        assert_eq!(
            set.cards[2].today,
            [fixtures::at(17, 5)],
            "◆ one today, 17:05"
        );
        assert!(set.cards[0].today.is_empty());
        assert_eq!(
            (set.missing, set.missing_cost.map(format::money).as_deref()),
            (3, Some("£0.15")),
            "3 short of a badge: ≈ £0.15 to buy them"
        );
        let zone = fixtures::zone();
        assert_eq!(format::clock(g.priced_at.unwrap(), g.now, zone), "15:21");
        assert_eq!(
            format::ago((g.now - g.priced_at.unwrap()).to_std().unwrap()),
            "2h ago"
        );
        assert_eq!(g.stale, None);
        assert_eq!(g.normal_range.map(|r| (r.0.minor, r.1.minor)), Some((4, 6)));
        assert_eq!(g.foil_range.map(|r| (r.0.minor, r.1.minor)), Some((35, 60)));
        assert_eq!(g.basis, Basis::List);
    }

    #[test]
    fn this_sessions_drops_of_it_and_their_value() {
        let data = fixtures::farming_alone();
        let g = ChosenGame::build(&data.snapshot(), fixtures::HEAVY_RAIN).unwrap();
        assert_eq!(
            g.this_session,
            [
                SessionCard {
                    at: fixtures::at(17, 5),
                    card: Told::Named {
                        name: "Madison".into(),
                        foil: false
                    },
                    copy: Some(2)
                },
                SessionCard {
                    at: fixtures::at(17, 23),
                    card: Told::Identifying,
                    copy: None
                }
            ],
            "2 this session: Madison at 17:05, and one at 17:23 still being identified"
        );
        let value = g.value_this_session.unwrap();
        assert_eq!(format::money(value.total), "£0.05");
        assert_eq!(
            value.unpriced, 1,
            "£0.05 this session · 1 card not priced yet"
        );
    }

    #[test]
    fn a_set_not_read_yet_shows_its_price_range() {
        let data = fixtures::building_hours();
        let g = ChosenGame::build(&data.snapshot(), fixtures::STRAY).unwrap();
        assert_eq!(
            g.doing,
            Doing::BuildingHours { others: 11 },
            "▷ building hours"
        );
        assert!((g.hours_to_go - 1.6).abs() < 1e-9, "needs 1.6h");
        assert_eq!(g.set, None, "The set · not read yet");
        let range = |r: Option<(Money, Money)>| {
            r.map(|(a, b)| format!("{} – {}", format::money(a), format::money(b)))
        };
        assert_eq!(range(g.normal_range).as_deref(), Some("£0.12 – £0.18"));
        assert_eq!(range(g.foil_range).as_deref(), Some("£0.81 – £2.55"));
        assert_eq!(g.tier, Tier::Priority(1), "◉ Priority #1");
    }

    #[test]
    fn stale_prices_say_their_age() {
        let data = fixtures::prices_paused();
        let g = ChosenGame::build(&data.snapshot(), fixtures::HEAVY_RAIN).unwrap();
        assert_eq!(g.stale.map(format::age).as_deref(), Some("8h"), "8h old");
    }

    #[test]
    fn every_state_a_game_can_be_in() {
        let data = fixtures::farming_alone();
        let s = data.snapshot();
        let doing = |id| ChosenGame::build(&s, id).unwrap().doing;
        assert_eq!(doing(fixtures::LIMBO), Doing::Queued);
        assert!(matches!(doing(fixtures::WARFRAME), Doing::SetAside(a) if a.times == 1));
        assert_eq!(doing(fixtures::COUNTER_STRIKE), Doing::Skipped);
        assert_eq!(
            doing(fixtures::HADES),
            Doing::Done {
                at: Some(fixtures::at(14, 35))
            }
        );
        assert_eq!(ChosenGame::build(&s, 1), None);

        let waiting = fixtures::waiting_for_hades();
        assert_eq!(
            ChosenGame::build(&waiting.snapshot(), fixtures::HEAVY_RAIN)
                .unwrap()
                .doing,
            Doing::Waiting
        );
        let paused = fixtures::paused();
        assert_eq!(
            ChosenGame::build(&paused.snapshot(), fixtures::HEAVY_RAIN)
                .unwrap()
                .doing,
            Doing::NextUp
        );
        let mut only = fixtures::farming_alone();
        only.prefs.priority_games = vec![fixtures::LIMBO];
        only.prefs.only_priority = true;
        assert_eq!(
            ChosenGame::build(&only.snapshot(), fixtures::WARFRAME)
                .unwrap()
                .doing,
            Doing::NotFarmed
        );
    }

    #[test]
    fn the_details_ask_for_its_cards_order_books() {
        let data = fixtures::farming_alone();
        let hashes = ChosenGame::market_hash_names(&data.snapshot(), fixtures::HEAVY_RAIN);
        assert_eq!(hashes.len(), 5);
        assert_eq!(hashes[2], "960910-Madison");
        assert!(ChosenGame::market_hash_names(&data.snapshot(), 1).is_empty());
    }

    #[test]
    fn the_end_of_the_library_chooses_the_last_game_done() {
        let data = fixtures::nothing_to_farm();
        let g = ChosenGame::build(&data.snapshot(), fixtures::VAMPIRE_SURVIVORS).unwrap();
        assert_eq!(
            g.doing,
            Doing::Done { at: Some(data.now) },
            "✓ done at 17:44"
        );
        let set = g.set.unwrap();
        assert_eq!(
            (set.have, set.cards.len(), set.spares),
            (4, 6, 1),
            "Set: 4 of 6 · 1 spare"
        );
        assert_eq!((g.received, g.total), (5, 5));
        assert_eq!((g.per_drop, g.left), (None, None));
    }
}
