// The farm queue as the user thinks about it: what's farmed first, what they
// don't mind about, what they skipped, and what's done. Pure data — built
// fresh from the latest farming status and preferences on every frame.

use game::{AppId, Game, SteamLibrary};
use preferences::{Preferences, Tier};
use session::Mode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Section {
    Priority,
    Indifferent,
    Skipped,
    Done,
}

#[derive(Debug, Clone)]
pub struct QueueEntry {
    pub game: Game,
    pub tier: Tier,
    /// Being played right now, and how.
    pub playing: Option<Mode>,
    /// Farmed at all: not when "only priority" is on and it isn't one.
    pub wanted: bool,
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

#[derive(Debug, Default)]
pub struct Queue {
    /// Non-empty sections, in farming order.
    pub sections: Vec<(Section, Vec<QueueEntry>)>,
}

impl Queue {
    /// `order` is the farmer's own order (app IDs); games it leaves out come
    /// after, most played first. `playing` is what's being played, and how;
    /// empty while farming is paused.
    pub fn build(
        library: &SteamLibrary,
        order: &[AppId],
        playing: &[AppId],
        mode: Option<Mode>,
        prefs: &Preferences,
    ) -> Self {
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
        let mut sections: Vec<(Section, Vec<QueueEntry>)> = [
            Section::Priority,
            Section::Indifferent,
            Section::Skipped,
            Section::Done,
        ]
        .into_iter()
        .map(|s| (s, Vec::new()))
        .collect();
        for g in games {
            let entry = QueueEntry {
                tier: prefs.tier(g.app_id),
                playing: mode.filter(|_| playing.contains(&g.app_id)),
                wanted: prefs.wants(g.app_id),
                game: g.clone(),
            };
            let i = sections
                .iter()
                .position(|(s, _)| *s == entry.section())
                .unwrap_or_default();
            sections[i].1.push(entry);
        }
        // Priority is the user's own order; done games go by name.
        sections[0].1.sort_by_key(|e| match e.tier {
            Tier::Priority(n) => n,
            _ => usize::MAX,
        });
        sections[3].1.sort_by_key(|e| e.game.name.to_lowercase());
        sections.retain(|(_, v)| !v.is_empty());
        Self { sections }
    }

    /// Every entry in display order.
    pub fn entries(&self) -> impl Iterator<Item = &QueueEntry> {
        self.sections.iter().flat_map(|(_, v)| v.iter())
    }

    pub fn get(&self, app_id: AppId) -> Option<&QueueEntry> {
        self.entries().find(|e| e.game.app_id == app_id)
    }

    pub fn is_empty(&self) -> bool {
        self.sections.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use game::{AppId, test_support::game};

    use super::*;

    fn ids(q: &Queue, section: Section) -> Vec<u32> {
        q.sections
            .iter()
            .find(|(s, _)| *s == section)
            .map(|(_, v)| v.iter().map(|e| e.game.app_id.0).collect())
            .unwrap_or_default()
    }

    #[test]
    fn groups_by_tier_in_the_farmers_order() {
        let library = SteamLibrary::new(vec![
            game(1, 5.0, 0, 3),
            game(2, 1.0, 0, 2),
            game(3, 8.0, 0, 1),
            game(4, 2.0, 0, 2),
            game(5, 9.0, 3, 0),
        ]);
        let prefs = Preferences {
            priority_games: vec![AppId(2), AppId(1)],
            skipped_games: vec![AppId(4)],
            ..Default::default()
        };
        let q = Queue::build(
            &library,
            &[2, 1, 3].map(AppId),
            &[AppId(2)],
            Some(Mode::Hours),
            &prefs,
        );

        assert_eq!(ids(&q, Section::Priority), [2, 1]);
        assert_eq!(ids(&q, Section::Indifferent), [3]);
        assert_eq!(ids(&q, Section::Skipped), [4]);
        assert_eq!(ids(&q, Section::Done), [5]);
        assert_eq!(q.get(AppId(2)).unwrap().playing, Some(Mode::Hours));
        assert_eq!(q.get(AppId(1)).unwrap().playing, None);
    }

    #[test]
    fn without_the_farmers_order_the_most_played_come_first() {
        let library = SteamLibrary::new(vec![game(1, 1.0, 0, 3), game(2, 7.0, 0, 2)]);
        let q = Queue::build(&library, &[], &[], None, &Preferences::default());
        assert_eq!(ids(&q, Section::Indifferent), [2, 1]);
    }

    #[test]
    fn with_only_priority_the_rest_arent_wanted() {
        let library = SteamLibrary::new(vec![game(1, 1.0, 0, 3), game(2, 7.0, 0, 2)]);
        let prefs = Preferences {
            priority_games: vec![AppId(1)],
            only_priority: true,
            ..Default::default()
        };
        let q = Queue::build(&library, &[AppId(1)], &[], None, &prefs);
        assert!(!q.get(AppId(2)).unwrap().wanted);
        assert!(q.get(AppId(1)).unwrap().wanted);
    }
}
