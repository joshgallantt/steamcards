// The farm queue as a flight plan: every game in the order it will be
// farmed, in the user's sections (priority, indifferent, skipped, done), each
// row with its state, drops as pips, what it's worth and when it should be
// done (docs/design/ui.md §2.1, §2.3). Pure data, built fresh each frame
// from the farmer's status, the forecast, the preferences and the prices.

use std::time::Duration;

use chrono::{DateTime, Utc};
use farming::{Mode, hours_to_go};
use library::Game;
use market::{Estimate, expected_per_drop, value_left};
use preferences::Tier;

use super::{
    now::Pips,
    screen::{Activity, Cell, Snapshot},
};

/// The widest a row's pips go: a game with more drops shows k/N only.
pub const MOST_PIPS: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Priority,
    Indifferent,
    Skipped,
    Done,
}

/// A row's glyph: what's happening to the game now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Glyph {
    /// ▶ played alone, its cards dropping.
    Farming,
    /// ▷ played with others, building hours.
    Hours,
    /// ‖ waiting while another device plays.
    Waiting,
    /// ✓ every card has dropped.
    Done,
    None,
}

/// A row's STATUS words (§5.4): farming, ready, needs 0.8h, set aside, next
/// up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RowStatus {
    Farming,
    /// Played with others, building the hours it still needs.
    BuildingHours(f64),
    Waiting,
    /// First in line while farming is stopped.
    NextUp,
    /// Has the hours its cards need.
    Ready,
    /// Short of 3 hours by this many.
    Needs(f64),
    /// Put behind the others after 10 hours without a drop.
    SetAside,
    /// Set aside twice: left alone for the rest of the session.
    LeftAlone,
    Skipped,
    /// Not farmed: "only priority" is on, and it isn't one.
    NotFarmed,
    Done,
}

/// What a section's games are doing, for its rule: "INDIFFERENT · 57 ·
/// farming".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionDoing {
    Farming,
    BuildingHours,
    Waiting,
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub game: Game,
    pub tier: Tier,
    /// Being played right now, and how.
    pub playing: Option<Mode>,
    /// Farmed at all: not when "only priority" is on and it isn't one.
    pub wanted: bool,
    /// The chosen game: its row is the selection bar.
    pub selected: bool,
    pub glyph: Glyph,
    pub status: RowStatus,
    pub pips: Pips,
    /// What a drop is likely worth, and all its drops left: `None` when
    /// there's nothing left to drop.
    pub per_drop: Option<Cell>,
    pub left: Option<Cell>,
    /// When its last card should drop, from now.
    pub done_in: Option<Duration>,
    /// When its last card dropped, if that was this session.
    pub finished_at: Option<DateTime<Utc>>,
}

impl QueueEntry {
    pub fn section(&self) -> Section {
        if !self.game.has_drops_left() {
            return Section::Done;
        }
        match self.tier {
            Tier::Priority(_) => Section::Priority,
            Tier::Indifferent => Section::Indifferent,
            Tier::Skip => Section::Skipped,
        }
    }
}

/// A section and its games, and what its rule says.
#[derive(Debug, Clone)]
pub struct SectionRows {
    pub section: Section,
    pub entries: Vec<QueueEntry>,
    /// What its games still to farm are worth: the sum of their ≈ LEFT, and
    /// how many aren't priced. `None` for skipped and done games, and
    /// before the wallet's currency is known.
    pub left: Option<Estimate>,
    pub doing: Option<SectionDoing>,
    /// Games done this session, for Done's rule.
    pub this_session: usize,
}

#[derive(Debug, Default)]
pub struct Queue {
    /// Every section in farming order, Priority, Indifferent, Skipped and
    /// Done, each even when empty.
    pub sections: Vec<SectionRows>,
    /// How wide the pips' column is: the most drops any game has, up to
    /// `MOST_PIPS`.
    pub pip_width: u32,
}

impl Queue {
    /// The queue as it stands: `selected` is the chosen game, by app ID.
    pub fn build(s: &Snapshot<'_>, selected: Option<u32>) -> Self {
        let library = s.library();
        let order = s.order();
        let activity = Activity::of(s);
        let place = |g: &Game| order.iter().position(|&id| id == g.app_id);
        let mut games: Vec<&Game> = library.games().iter().collect();
        games.sort_by(|a, b| {
            let (pa, pb) = (place(a), place(b));
            pa.is_none()
                .cmp(&pb.is_none())
                .then(pa.cmp(&pb))
                .then(b.hours.total_cmp(&a.hours))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        let finished = |id: u32| {
            s.session()
                .and_then(|ss| ss.finished.iter().find(|f| f.app_id == id))
                .map(|f| f.at)
        };
        let mut sections: Vec<SectionRows> = [
            Section::Priority,
            Section::Indifferent,
            Section::Skipped,
            Section::Done,
        ]
        .into_iter()
        .map(|section| SectionRows {
            section,
            entries: Vec::new(),
            left: None,
            doing: None,
            this_session: 0,
        })
        .collect();
        for g in games {
            let entry = entry(s, &activity, g, selected, finished(g.app_id));
            let i = match entry.section() {
                Section::Priority => 0,
                Section::Indifferent => 1,
                Section::Skipped => 2,
                Section::Done => 3,
            };
            sections[i].entries.push(entry);
        }
        // Priority is the user's own order; done games, the newest first,
        // then those done before this session, by name.
        sections[0].entries.sort_by_key(|e| match e.tier {
            Tier::Priority(n) => n,
            _ => usize::MAX,
        });
        sections[3].entries.sort_by(|a, b| {
            b.finished_at
                .cmp(&a.finished_at)
                .then_with(|| a.game.name.to_lowercase().cmp(&b.game.name.to_lowercase()))
        });
        for rows in &mut sections {
            rows.doing = doing(&rows.entries);
            rows.this_session = rows
                .entries
                .iter()
                .filter(|e| e.finished_at.is_some())
                .count();
            if matches!(rows.section, Section::Priority | Section::Indifferent) {
                let ids: Vec<u32> = rows
                    .entries
                    .iter()
                    .filter(|e| order.contains(&e.game.app_id))
                    .map(|e| e.game.app_id)
                    .collect();
                rows.left = s
                    .wallet
                    .map(|w| value_left(library, &ids, s.prices, s.basis, &w));
            }
        }
        let pip_width = sections
            .iter()
            .flat_map(|r| &r.entries)
            .map(|e| e.game.drops.total())
            .max()
            .unwrap_or(0)
            .min(MOST_PIPS);
        Self {
            sections,
            pip_width,
        }
    }

    /// Every entry in display order.
    pub fn entries(&self) -> impl Iterator<Item = &QueueEntry> {
        self.sections.iter().flat_map(|r| r.entries.iter())
    }

    pub fn get(&self, app_id: u32) -> Option<&QueueEntry> {
        self.entries().find(|e| e.game.app_id == app_id)
    }

    pub fn section(&self, section: Section) -> &SectionRows {
        &self.sections[match section {
            Section::Priority => 0,
            Section::Indifferent => 1,
            Section::Skipped => 2,
            Section::Done => 3,
        }]
    }

    pub fn is_empty(&self) -> bool {
        self.entries().next().is_none()
    }

    /// Card drops still to come across the games that are farmed.
    pub fn drops_to_go(&self) -> u32 {
        self.entries()
            .filter(|e| e.wanted && e.section() != Section::Skipped)
            .map(|e| e.game.drops.remaining)
            .sum()
    }
}

fn entry(
    s: &Snapshot<'_>,
    activity: &Activity,
    g: &Game,
    selected: Option<u32>,
    finished_at: Option<DateTime<Utc>>,
) -> QueueEntry {
    let tier = s.prefs.tier(g.app_id);
    let playing = s.mode().filter(|_| s.playing().contains(&g.app_id));
    let first = s.order().first() == Some(&g.app_id);
    let set_aside = s
        .status
        .and_then(|st| st.set_aside.iter().find(|a| a.app_id == g.app_id));
    let wanted = s.prefs.wants(g.app_id);
    let (glyph, status) = if !g.has_drops_left() {
        (Glyph::Done, RowStatus::Done)
    } else if tier == Tier::Skip {
        (Glyph::None, RowStatus::Skipped)
    } else if !wanted {
        (Glyph::None, RowStatus::NotFarmed)
    } else {
        match (playing, activity) {
            (Some(Mode::Cards), _) => (Glyph::Farming, RowStatus::Farming),
            (Some(Mode::Hours), _) => (Glyph::Hours, RowStatus::BuildingHours(hours_to_go(g))),
            (None, Activity::Waiting { .. }) if first => (Glyph::Waiting, RowStatus::Waiting),
            (None, a) if first && a.holds_still() => (Glyph::None, RowStatus::NextUp),
            _ => match set_aside {
                Some(a) if a.times >= 2 => (Glyph::None, RowStatus::LeftAlone),
                Some(_) => (Glyph::None, RowStatus::SetAside),
                None if hours_to_go(g) > 0.0 => (Glyph::None, RowStatus::Needs(hours_to_go(g))),
                None => (Glyph::None, RowStatus::Ready),
            },
        }
    };
    let set = s.prices.sets.get(&g.app_id);
    let (per_drop, left) = if g.has_drops_left() && tier != Tier::Skip {
        let each = match (set, s.wallet) {
            (_, None) | (None, _) => Cell::Pending,
            (Some(set), Some(w)) => match expected_per_drop(set, s.basis, &w) {
                Some(value) => Cell::Value { value, stale: None },
                None if set.retry_at.is_some() => Cell::Failed,
                None => Cell::NoMarket,
            },
        };
        let left = match (each, s.wallet) {
            (Cell::Value { .. }, Some(w)) => Cell::Value {
                value: value_left(s.library(), &[g.app_id], s.prices, s.basis, &w).value,
                stale: None,
            },
            (other, _) => other,
        };
        (Some(each), Some(left))
    } else {
        (None, None)
    };
    QueueEntry {
        game: g.clone(),
        tier,
        playing,
        wanted,
        selected: selected == Some(g.app_id),
        glyph,
        status,
        pips: Pips::of(g, s.drops()),
        per_drop,
        left,
        done_in: s.done_in(g.app_id),
        finished_at,
    }
}

/// What a section's rule says it's doing: farming a game alone, building
/// hours, or waiting.
fn doing(entries: &[QueueEntry]) -> Option<SectionDoing> {
    let any = |glyph| entries.iter().any(|e| e.glyph == glyph);
    if any(Glyph::Farming) {
        Some(SectionDoing::Farming)
    } else if any(Glyph::Hours) {
        Some(SectionDoing::BuildingHours)
    } else if any(Glyph::Waiting) {
        Some(SectionDoing::Waiting)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use market::Money;
    use preferences::Preferences;

    use super::*;
    use crate::{
        tui::format,
        viewmodel::{Run, fixtures},
    };

    fn names(q: &Queue, section: Section) -> Vec<String> {
        q.section(section)
            .entries
            .iter()
            .map(|e| e.game.name.clone())
            .collect()
    }

    fn cell(c: Option<Cell>) -> String {
        match c {
            Some(Cell::Value { value, .. }) => format::money(value),
            Some(other) => format!("{other:?}"),
            None => String::new(),
        }
    }

    #[test]
    fn the_queue_is_the_farm_order_in_its_sections() {
        let data = fixtures::farming_alone();
        let q = Queue::build(&data.snapshot(), Some(fixtures::HEAVY_RAIN));
        assert!(
            names(&q, Section::Priority).is_empty(),
            "nothing ranked yet"
        );
        let indifferent = names(&q, Section::Indifferent);
        assert_eq!(indifferent.len(), 57);
        assert_eq!(
            indifferent[..3],
            ["Heavy Rain", "LIMBO", "Oxygen Not Included"]
        );
        assert_eq!(indifferent.last().map(String::as_str), Some("Warframe"));
        assert_eq!(names(&q, Section::Skipped), ["Counter-Strike 2"]);
        assert_eq!(
            names(&q, Section::Done),
            [
                "Gorogoa",
                "Celeste",
                "Hades",
                "Inscryption",
                "Hollow Knight"
            ],
            "newest first"
        );
        assert_eq!(q.section(Section::Done).this_session, 5);
        assert_eq!(q.pip_width, 15, "the most drops, Little Nightmares II's");
        assert_eq!(q.drops_to_go(), 236);
        assert!(!q.is_empty());
    }

    #[test]
    fn each_row_has_its_state_pips_values_and_time() {
        let data = fixtures::farming_alone();
        let q = Queue::build(&data.snapshot(), Some(fixtures::HEAVY_RAIN));
        let heavy_rain = q.get(fixtures::HEAVY_RAIN).unwrap();
        assert!(heavy_rain.selected);
        assert_eq!(
            (heavy_rain.glyph, heavy_rain.status),
            (Glyph::Farming, RowStatus::Farming)
        );
        assert_eq!(
            heavy_rain.pips,
            Pips {
                before: 1,
                today: 2,
                foils: 0,
                total: 4
            }
        );
        assert_eq!(cell(heavy_rain.per_drop), "£0.05");
        assert_eq!(cell(heavy_rain.left), "£0.05");
        assert_eq!(format::estimate(heavy_rain.done_in.unwrap()), "25m");
        assert_eq!(heavy_rain.playing, Some(Mode::Cards));

        let limbo = q.get(fixtures::LIMBO).unwrap();
        assert!(!limbo.selected);
        assert_eq!((limbo.glyph, limbo.status), (Glyph::None, RowStatus::Ready));
        assert_eq!(
            (cell(limbo.per_drop), cell(limbo.left)),
            ("£0.06".into(), "£0.12".into())
        );
        assert_eq!(format::estimate(limbo.done_in.unwrap()), "1h 30m");

        let tcg = q
            .entries()
            .find(|e| e.game.name == "TCG Card Shop Simulator")
            .unwrap();
        let RowStatus::Needs(h) = tcg.status else {
            panic!("{:?}", tcg.status);
        };
        assert_eq!(format!("needs {h:.1}h"), "needs 0.8h");

        let warframe = q.get(fixtures::WARFRAME).unwrap();
        assert_eq!(warframe.status, RowStatus::SetAside);
        assert_eq!(format::estimate(warframe.done_in.unwrap()), "4d 21h");

        let cs2 = q.get(fixtures::COUNTER_STRIKE).unwrap();
        assert_eq!(
            (cs2.status, cs2.per_drop, cs2.done_in),
            (RowStatus::Skipped, None, None)
        );

        let hades = q.get(fixtures::HADES).unwrap();
        assert_eq!((hades.glyph, hades.status), (Glyph::Done, RowStatus::Done));
        assert_eq!(hades.finished_at, Some(fixtures::at(14, 35)), "✓ 14:35");
        assert_eq!(hades.pips.foils, 1);
    }

    #[test]
    fn each_sections_rule_carries_its_value_left() {
        let data = fixtures::farming_alone();
        let q = Queue::build(&data.snapshot(), None);
        let indifferent = q.section(Section::Indifferent);
        assert_eq!(
            indifferent.doing,
            Some(SectionDoing::Farming),
            "INDIFFERENT · 57 · farming"
        );
        assert_eq!(format::about(indifferent.left.unwrap().value), "≈ £15.34");
        let priority = q.section(Section::Priority);
        assert_eq!(
            priority.left.unwrap().value,
            Money::zero(market::Currency::GBP)
        );
        assert_eq!(priority.doing, None);
        assert_eq!(q.section(Section::Skipped).left, None);
    }

    #[test]
    fn building_hours_marks_the_group_in_both_sections() {
        let data = fixtures::building_hours();
        let q = Queue::build(&data.snapshot(), Some(fixtures::STRAY));
        assert_eq!(names(&q, Section::Priority), ["Stray", "LIMBO"]);
        let priority = q.section(Section::Priority);
        assert_eq!(
            priority.doing,
            Some(SectionDoing::BuildingHours),
            "PRIORITY · 2 · building hours"
        );
        assert_eq!(format::about(priority.left.unwrap().value), "≈ £0.26");
        let indifferent = q.section(Section::Indifferent);
        assert_eq!(indifferent.entries.len(), 55);
        assert_eq!(indifferent.doing, Some(SectionDoing::BuildingHours));
        assert_eq!(format::about(indifferent.left.unwrap().value), "≈ £15.08");
        let stray = q.get(fixtures::STRAY).unwrap();
        assert_eq!(stray.glyph, Glyph::Hours);
        assert_eq!(format::estimate(stray.done_in.unwrap()), "2h");
        assert_eq!(stray.tier, Tier::Priority(1));
        let heavy_rain = q.get(fixtures::HEAVY_RAIN).unwrap();
        assert_eq!(heavy_rain.glyph, Glyph::None, "no longer played");
        assert_eq!(format::estimate(heavy_rain.done_in.unwrap()), "3h 30m");
    }

    #[test]
    fn the_first_in_line_waits_or_is_next_up() {
        let waiting = fixtures::waiting_for_hades();
        let q = Queue::build(&waiting.snapshot(), None);
        let first = q.get(fixtures::HEAVY_RAIN).unwrap();
        assert_eq!(
            (first.glyph, first.status),
            (Glyph::Waiting, RowStatus::Waiting)
        );
        assert_eq!(
            q.section(Section::Indifferent).doing,
            Some(SectionDoing::Waiting)
        );

        let paused = fixtures::paused();
        let q = Queue::build(&paused.snapshot(), None);
        let first = q.get(fixtures::HEAVY_RAIN).unwrap();
        assert_eq!(
            (first.glyph, first.status),
            (Glyph::None, RowStatus::NextUp)
        );
        assert_eq!(q.section(Section::Indifferent).doing, None);
    }

    #[test]
    fn the_first_minutes_sum_what_is_priced_so_far() {
        let data = fixtures::first_minutes();
        let q = Queue::build(&data.snapshot(), None);
        let left = q.section(Section::Indifferent).left.unwrap();
        assert_eq!(format::so_far(&left), "≈ £2.37 so far · 48 unpriced");
        let dead_by_daylight = q
            .entries()
            .find(|e| e.game.name == "Dead by Daylight")
            .unwrap();
        assert_eq!(dead_by_daylight.per_drop, Some(Cell::Pending), "…");
        assert_eq!(names(&q, Section::Done).len(), 0, "none yet");
    }

    #[test]
    fn only_priority_leaves_the_rest_unfarmed() {
        let mut data = fixtures::farming_alone();
        data.prefs = Preferences {
            priority_games: vec![fixtures::LIMBO],
            only_priority: true,
            ..Default::default()
        };
        data.run = Run::Paused;
        let q = Queue::build(&data.snapshot(), None);
        assert_eq!(
            q.get(fixtures::HEAVY_RAIN).unwrap().status,
            RowStatus::NotFarmed
        );
        assert!(!q.get(fixtures::HEAVY_RAIN).unwrap().wanted);
        assert_eq!(q.drops_to_go(), 2);
    }

    #[test]
    fn a_game_set_aside_twice_is_left_alone() {
        let mut data = fixtures::farming_alone();
        data.status.set_aside[0].times = 2;
        let q = Queue::build(&data.snapshot(), None);
        assert_eq!(
            q.get(fixtures::WARFRAME).unwrap().status,
            RowStatus::LeftAlone
        );
    }
}
