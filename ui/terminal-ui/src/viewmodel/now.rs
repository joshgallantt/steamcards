// Now: what's being farmed and how, its drops, the last card and the next,
// and when its last card should drop; or, in other states, what's happening
// and what happens next (docs/design/ui.md §2.1). The Now panel at L, and
// the Now line in Progress below it.

use std::time::Duration;

use chrono::{DateTime, FixedOffset, Utc};
use farming::{Drop, DropCard, hours_to_go};
use library::Game;
use market::{Money, value_of};
use preferences::Tier;

use super::screen::{Activity, Snapshot};

/// A game's drops, one pip each: had before this session, dropped this
/// session (the foils among them apart), and still to come.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pips {
    pub before: u32,
    pub today: u32,
    /// This session's drops that were foils.
    pub foils: u32,
    pub total: u32,
}

impl Pips {
    /// `game`'s drops, with `drops` this session's.
    pub fn of(game: &Game, drops: &[Drop]) -> Self {
        let mine = drops.iter().filter(|d| d.app_id == game.app_id);
        let today = u32::try_from(mine.clone().count()).unwrap_or(u32::MAX);
        let foils = u32::try_from(mine.filter(|d| d.card.is_foil()).count()).unwrap_or(u32::MAX);
        let today = today.min(game.drops.received);
        Self {
            before: game.drops.received - today,
            today,
            foils: foils.min(today),
            total: game.drops.total(),
        }
    }
}

/// The Now panel and line.
#[derive(Debug, Clone, PartialEq)]
pub struct Now {
    pub activity: Activity,
    /// The game farmed alone.
    pub game: Option<GameNow>,
    /// The games building hours together.
    pub group: Option<Group>,
    /// The first game in the farm order: what farming carries on with.
    pub next: Option<String>,
    /// When the farmer next looks at the cards, or tries again.
    pub next_look: Option<DateTime<Utc>>,
    /// How often the game farmed alone has its cards looked at.
    pub look_every: Option<Duration>,
    pub now: DateTime<Utc>,
    pub zone: FixedOffset,
}

/// The game farmed alone.
#[derive(Debug, Clone, PartialEq)]
pub struct GameNow {
    pub app_id: u32,
    pub name: String,
    pub pips: Pips,
    pub received: u32,
    pub total: u32,
    pub remaining: u32,
    /// On it since: when the stretch going on began.
    pub since: Option<DateTime<Utc>>,
    /// Its last drop this session, and which card it was.
    pub last_drop: Option<LastDrop>,
    pub next_card: NextCard,
    /// No card has dropped this session yet: when the first should.
    pub first_card: Option<DateTime<Utc>>,
    /// When its last card should drop.
    pub last_card: Option<DateTime<Utc>>,
    /// The game farmed after it.
    pub then: Option<String>,
}

/// A card that dropped, as far as it's known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastDrop {
    pub at: DateTime<Utc>,
    pub card: Told,
}

/// What's known of which card a drop was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Told {
    /// Being found out: "⠋ finding out which card".
    Identifying,
    Named {
        name: String,
        foil: bool,
    },
    /// Nothing could tell: "couldn't tell which card".
    Unknown,
}

impl Told {
    pub fn of(card: &DropCard) -> Self {
        match card {
            DropCard::Identifying => Self::Identifying,
            DropCard::Unknown => Self::Unknown,
            DropCard::Identified(asset) => Self::Named {
                name: asset.name.clone(),
                foil: asset.foil,
            },
            DropCard::NameOnly { name, foil } => Self::Named {
                name: name.clone(),
                foil: *foil,
            },
        }
    }
}

/// What the next card could be: one of the set's cards, how many of them
/// are new to the account, and what they sell for on the basis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NextCard {
    /// The set's size; 0 while its card page hasn't been read.
    pub cards: usize,
    pub new: usize,
    pub normal: Option<(Money, Money)>,
    pub foil: Option<(Money, Money)>,
}

/// Games building hours together, the lead first.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub games: Vec<GroupGame>,
    /// When the lead reaches 3 hours.
    pub lead_ready: DateTime<Utc>,
    /// Played together since: when the stretch going on began.
    pub since: Option<DateTime<Utc>>,
}

/// A game of the group, and the hours it still needs.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupGame {
    pub name: String,
    pub rank: Option<usize>,
    pub hours: f64,
    pub to_go: f64,
}

impl Now {
    pub fn build(s: &Snapshot<'_>) -> Self {
        let activity = Activity::of(s);
        let order = s.order();
        let game = s.farming_alone().map(|g| game_now(s, g));
        let group = match &activity {
            Activity::BuildingHours { app_ids } => group(s, app_ids),
            _ => None,
        };
        Self {
            next: order.first().map(|&id| s.name(id)),
            next_look: s.status.and_then(|st| st.next_look),
            look_every: s
                .status
                .and_then(|st| st.look_every)
                .filter(|_| game.is_some()),
            game,
            group,
            activity,
            now: s.now,
            zone: s.zone,
        }
    }
}

fn game_now(s: &Snapshot<'_>, g: &Game) -> GameNow {
    let drops = s.drops();
    let session = s.session();
    let order = s.order();
    let then = order
        .iter()
        .position(|&id| id == g.app_id)
        .and_then(|i| order.get(i + 1))
        .map(|&id| s.name(id));
    let done_in = s.done_in(g.app_id);
    let first_card = done_in
        .filter(|_| drops.is_empty() && g.drops.remaining > 0)
        .map(|d| s.now + d / g.drops.remaining);
    GameNow {
        app_id: g.app_id,
        name: g.name.clone(),
        pips: Pips::of(g, drops),
        received: g.drops.received,
        total: g.drops.total(),
        remaining: g.drops.remaining,
        since: session.and_then(|ss| {
            ss.stretches
                .iter()
                .rev()
                .take_while(|st| st.app_ids.contains(&g.app_id))
                .last()
                .map(|st| st.from)
        }),
        last_drop: drops
            .iter()
            .rev()
            .find(|d| d.app_id == g.app_id)
            .map(|d| LastDrop {
                at: d.at,
                card: Told::of(&d.card),
            }),
        next_card: next_card(s, g),
        first_card,
        last_card: done_in.map(|d| s.now + d),
        then,
    }
}

/// What the next card could be, from the set and its prices.
fn next_card(s: &Snapshot<'_>, g: &Game) -> NextCard {
    let set = s.prices.sets.get(&g.app_id);
    let range = |foil: bool| {
        let (set, wallet) = (set?, s.wallet?);
        let border = if foil { &set.foil } else { &set.normal };
        let values: Vec<Money> = border
            .iter()
            .filter_map(|c| value_of(&c.price, s.basis.still_to_drop(), &wallet))
            .collect();
        let low = values.iter().min_by_key(|m| m.minor)?;
        let high = values.iter().max_by_key(|m| m.minor)?;
        Some((*low, *high))
    };
    NextCard {
        cards: g.cards.len(),
        new: g.missing().count(),
        normal: range(false),
        foil: range(true),
    }
}

/// The games building hours, the lead first, and when it reaches 3 hours.
fn group(s: &Snapshot<'_>, app_ids: &[u32]) -> Option<Group> {
    let library = s.library();
    let games: Vec<GroupGame> = app_ids
        .iter()
        .filter_map(|&id| library.game(id))
        .map(|g| GroupGame {
            name: g.name.clone(),
            rank: match s.prefs.tier(g.app_id) {
                Tier::Priority(n) => Some(n),
                _ => None,
            },
            hours: g.hours,
            to_go: hours_to_go(g),
        })
        .collect();
    let lead = games.first()?;
    let lead_ready = s.now + Duration::from_secs_f64(lead.to_go * 3600.0);
    let since = s
        .session()
        .and_then(|ss| ss.stretches.last())
        .filter(|st| st.to.is_none() && st.app_ids.iter().any(|id| app_ids.contains(id)))
        .map(|st| st.from);
    Some(Group {
        games,
        lead_ready,
        since,
    })
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use market::Currency;

    use super::*;
    use crate::{tui::format, viewmodel::fixtures};

    #[test]
    fn heavy_rain_is_farmed_alone() {
        let data = fixtures::farming_alone();
        let now = Now::build(&data.snapshot());
        let zone = fixtures::zone();
        let g = now.game.clone().unwrap();
        assert_eq!(g.name, "Heavy Rain");
        assert_eq!(g.app_id, fixtures::HEAVY_RAIN);
        assert_eq!(
            g.pips,
            Pips {
                before: 1,
                today: 2,
                foils: 0,
                total: 4
            },
            "●◆◆○"
        );
        assert_eq!(
            (g.received, g.total, g.remaining),
            (3, 4, 1),
            "3 of 4 drops · 1 to go"
        );
        assert_eq!(g.since, Some(fixtures::at(16, 53)), "since 16:53");
        assert_eq!(
            format::duration((now.now - g.since.unwrap()).to_std().unwrap()),
            "38m"
        );
        assert_eq!(
            g.last_drop,
            Some(LastDrop {
                at: fixtures::at(17, 23),
                card: Told::Identifying
            }),
            "8m ago, at 17:23 · ⠋ finding out which card"
        );
        assert_eq!(g.first_card, None);
        assert_eq!(
            format::estimate_at(
                now.now,
                (g.last_card.unwrap() - now.now).to_std().unwrap(),
                zone
            ),
            "17:55",
            "last card ≈ 17:55"
        );
        assert_eq!(g.then.as_deref(), Some("LIMBO"), "then LIMBO");
        assert_eq!(format::countdown(now.next_look.unwrap(), now.now), "in 4m");
        assert_eq!(
            now.look_every,
            Some(Duration::from_secs(300)),
            "every 5 min"
        );
        assert_eq!(now.next.as_deref(), Some("Heavy Rain"));
        assert_eq!(now.group, None);
        assert!(matches!(now.activity, Activity::Farming { .. }));
    }

    #[test]
    fn the_next_card_is_one_of_its_set() {
        let data = fixtures::farming_alone();
        let g = Now::build(&data.snapshot()).game.unwrap();
        let pence = |m: (Money, Money)| (m.0.minor, m.1.minor);
        assert_eq!(
            (g.next_card.cards, g.next_card.new),
            (5, 3),
            "one of its 5 cards, 3 of them new to you"
        );
        assert_eq!(g.next_card.normal.map(pence), Some((4, 6)), "£0.04 – £0.06");
        assert_eq!(g.next_card.foil.map(pence), Some((35, 60)), "£0.35 – £0.60");
        assert_eq!(g.next_card.normal.unwrap().0.currency, Currency::GBP);
    }

    #[test]
    fn the_first_card_of_a_session_is_expected_at_its_rate() {
        let data = fixtures::first_minutes();
        let now = Now::build(&data.snapshot());
        let g = now.game.unwrap();
        let zone = fixtures::zone();
        assert_eq!(g.name, "Hollow Knight");
        assert_eq!(
            format::estimate_at(
                now.now,
                (g.first_card.unwrap() - now.now).to_std().unwrap(),
                zone
            ),
            "09:45",
            "first card ≈ 09:45"
        );
        assert_eq!(format::countdown(now.next_look.unwrap(), now.now), "in 12m");
        assert_eq!((g.next_card.cards, g.next_card.new), (7, 6));
        assert_eq!(g.last_drop, None);
    }

    #[test]
    fn building_hours_lists_the_group_lead_first() {
        let data = fixtures::building_hours();
        let now = Now::build(&data.snapshot());
        let group = now.group.unwrap();
        assert_eq!(group.games.len(), 12);
        let stray = &group.games[0];
        assert_eq!((stray.name.as_str(), stray.rank), ("Stray", Some(1)));
        assert!((stray.to_go - 1.6).abs() < 1e-9 && (stray.hours - 1.4).abs() < 1e-9);
        assert_eq!(
            format::estimate_at(
                now.now,
                (group.lead_ready - now.now).to_std().unwrap(),
                fixtures::zone()
            ),
            "19:00",
            "Stray has 3h ≈ 19:00"
        );
        assert_eq!(group.since, Some(fixtures::at(17, 24)), "since 17:24");
        assert_eq!(now.game, None);
        assert_eq!(now.look_every, None);
    }

    #[test]
    fn waiting_says_what_farming_carries_on_with() {
        let data = fixtures::waiting_for_hades();
        let now = Now::build(&data.snapshot());
        assert_eq!(
            now.next.as_deref(),
            Some("Heavy Rain"),
            "Heavy Rain is next"
        );
        assert_eq!(now.game, None);
        let data = fixtures::reconnecting();
        let now = Now::build(&data.snapshot());
        assert_eq!(
            now.next_look,
            Some(fixtures::now() + TimeDelta::seconds(42))
        );
    }

    #[test]
    fn a_foil_this_session_is_a_star() {
        let data = fixtures::farming_alone();
        let hades = data.status.library.game(fixtures::HADES).unwrap();
        assert_eq!(
            Pips::of(hades, &data.status.session.drops),
            Pips {
                before: 0,
                today: 4,
                foils: 1,
                total: 4
            }
        );
        assert_eq!(Told::of(&DropCard::Unknown), Told::Unknown);
        assert_eq!(
            Told::of(&DropCard::NameOnly {
                name: "Madison".into(),
                foil: false
            }),
            Told::Named {
                name: "Madison".into(),
                foil: false
            }
        );
    }
}
