//! The session: kept from one run of the farmer to the next, with what the
//! farmer knows beside it. It records every card that dropped, one drop per
//! copy, and then which card each was; what was played, and how; the games
//! finished; and the first forecast.
//!
//! Which card dropped is told as Steam's own site tells it (research:
//! market-and-session.md, section 2): the items Steam announced, described
//! by `DescribeCards`. Failing that, the game's card page: a count that went
//! up since the last look names the card. Failing that, it isn't known.

use std::{
    collections::HashMap,
    iter,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Utc};
use library::{Card, CardAsset, Game, SteamLibrary};
use preferences::Preferences;

use crate::{
    Drop, DropCard, FarmingSession, Finished, Mode, NewItem, SetAside, Stretch, forecast::forecast,
    ranking::farm_order,
};

/// Keeps the farming session from one run of [`FarmCards`](crate::FarmCards)
/// to the next, through pauses, until [`EndSession`](crate::EndSession) ends
/// it. The two use cases share one; nothing else reaches into it. A session
/// is never kept on disk: quitting steamcards ends it too.
#[derive(Default)]
pub struct SessionKeeper {
    current: Mutex<Option<Arc<Mutex<Kept>>>>,
}

impl SessionKeeper {
    /// The session going on, or a new one starting `now`.
    pub(crate) fn current(&self, now: DateTime<Utc>) -> Arc<Mutex<Kept>> {
        self.current
            .lock()
            .unwrap()
            .get_or_insert_with(|| Arc::new(Mutex::new(Kept::new(now))))
            .clone()
    }

    /// Ends the session: the next run starts a new one. A run still going
    /// carries on with the old one, on its own.
    pub(crate) fn end(&self) {
        self.current.lock().unwrap().take();
    }
}

/// What the farmer keeps through a session: the session itself, and beside
/// it what should outlast a pause.
pub(crate) struct Kept {
    pub(crate) session: FarmingSession,
    /// The library as the farmer sees it: hours counted as they're played,
    /// drops as they land, card sets as last looked at.
    pub(crate) library: SteamLibrary,
    /// Whether the library has been read yet: until it has, there's nothing
    /// to tell drops by.
    read: bool,
    /// Hours counted here while playing. The badge pages take a while to
    /// show them (about half an hour), so these are a floor under what the
    /// pages say.
    counted: HashMap<u32, f64>,
    /// Games put behind the others after a long while without a drop.
    pub(crate) set_aside: Vec<SetAside>,
    /// Items Steam announced that nothing has asked about yet. Each waits
    /// for a drop of its game, or of any game when Steam didn't say which.
    announced: Vec<NewItem>,
}

/// Drops that one look at a game, or one read of the library, found.
#[derive(Debug)]
pub(crate) struct Found {
    pub(crate) app_id: u32,
    /// Where the first is in the session's drops; the rest follow it.
    first: usize,
    pub(crate) count: usize,
    /// The set as the farmer knew it before them: empty when it didn't.
    before: Vec<Card>,
    /// The set the look found, with them in it. The badge pages don't show
    /// sets, so a read has none.
    after: Option<Vec<Card>>,
}

impl Kept {
    fn new(now: DateTime<Utc>) -> Self {
        Self {
            session: FarmingSession {
                started_at: now,
                ..Default::default()
            },
            library: SteamLibrary::default(),
            read: false,
            counted: HashMap::new(),
            set_aside: Vec::new(),
            announced: Vec::new(),
        }
    }

    /// Takes in a fresh read of the library, keeping the hours counted here
    /// and the card sets already seen, and records the drops it shows. The
    /// session's first read is what it starts from.
    pub(crate) fn take(&mut self, fresh: SteamLibrary, now: DateTime<Utc>) -> Vec<Found> {
        let mut games = Vec::with_capacity(fresh.games().len());
        let mut found = Vec::new();
        for game in fresh.games() {
            let game = self.merged(game.clone());
            if self.read {
                found.extend(self.record(&game, None, now));
            }
            games.push(game);
        }
        self.library = SteamLibrary::new(games);
        if !self.read {
            self.read = true;
            self.session.drops_left_at_start = Some(self.library.drops_left());
            self.session.games_at_start = Some(self.library.with_drops_left().count() as u32);
        }
        found
    }

    /// Takes in a fresh look at one game: the game as the farmer now sees
    /// it, and the drops the look found.
    pub(crate) fn update(&mut self, fresh: Game, now: DateTime<Utc>) -> (Game, Option<Found>) {
        let set = (!fresh.cards.is_empty()).then(|| fresh.cards.clone());
        let game = self.merged(fresh);
        let found = self.record(&game, set, now);
        self.library.update(game.clone());
        (game, found)
    }

    fn merged(&self, mut fresh: Game) -> Game {
        if let Some(&hours) = self.counted.get(&fresh.app_id) {
            fresh.hours = fresh.hours.max(hours);
        }
        if let Some(known) = self.library.game(fresh.app_id) {
            if fresh.cards.is_empty() {
                fresh.cards = known.cards.clone();
            }
            // Cards never undrop: a page showing more to come than the
            // farmer has already seen is behind, and would drop them twice.
            if fresh.drops.remaining > known.drops.remaining {
                fresh.drops = known.drops;
            }
        }
        fresh
    }

    /// Records a drop for each card `game`'s drops left went down by since
    /// the farmer last saw it, each still being found out.
    fn record(
        &mut self,
        game: &Game,
        after: Option<Vec<Card>>,
        now: DateTime<Utc>,
    ) -> Option<Found> {
        let before = self.library.game(game.app_id)?;
        let count = before.drops.remaining.saturating_sub(game.drops.remaining);
        if count == 0 {
            return None;
        }
        let found = Found {
            app_id: game.app_id,
            first: self.session.drops.len(),
            count: count as usize,
            before: before.cards.clone(),
            after,
        };
        self.session.drops.extend(iter::repeat_n(
            Drop {
                at: now,
                app_id: game.app_id,
                card: DropCard::Identifying,
                copy: None,
            },
            found.count,
        ));
        if !game.has_drops_left() {
            self.session.finished.push(Finished {
                app_id: game.app_id,
                at: now,
            });
        }
        // It drops after all.
        self.set_aside.retain(|s| s.app_id != game.app_id);
        Some(found)
    }

    /// Counts `played` towards each game's hours.
    pub(crate) fn count(&mut self, app_ids: &[u32], played: Duration) {
        for &app_id in app_ids {
            let Some(mut game) = self.library.game(app_id).cloned() else {
                continue;
            };
            game.hours += played.as_secs_f64() / 3600.0;
            self.counted.insert(app_id, game.hours);
            self.library.update(game);
        }
    }

    pub(crate) fn name(&self, app_id: u32) -> String {
        self.library
            .game(app_id)
            .map_or_else(|| format!("app {app_id}"), |g| g.name.clone())
    }

    /// Starts a stretch of playing these games, this way, from `now`: where
    /// it is, to stop it by.
    pub(crate) fn start(&mut self, app_ids: &[u32], mode: Mode, now: DateTime<Utc>) -> usize {
        self.session.stretches.push(Stretch {
            app_ids: app_ids.to_vec(),
            mode,
            from: now,
            to: None,
        });
        self.session.stretches.len() - 1
    }

    /// Ends a stretch, if it goes on.
    pub(crate) fn stop(&mut self, stretch: usize, now: DateTime<Utc>) {
        if let Some(s) = self.session.stretches.get_mut(stretch) {
            s.to.get_or_insert(now);
        }
    }

    /// Puts a game behind the others once more: how often it has been.
    pub(crate) fn set_aside(&mut self, app_id: u32, now: DateTime<Utc>) -> u8 {
        match self.set_aside.iter_mut().find(|s| s.app_id == app_id) {
            Some(s) => {
                s.times = s.times.saturating_add(1);
                s.since = now;
                s.times
            }
            None => {
                self.set_aside.push(SetAside {
                    app_id,
                    times: 1,
                    since: now,
                });
                1
            }
        }
    }

    /// Keeps the items Steam announced, each once, until a drop they may be.
    pub(crate) fn keep_announced(&mut self, items: Vec<NewItem>) {
        for item in items {
            let known = self.announced.iter().any(|i| i.asset_id == item.asset_id)
                || self.session.drops.iter().any(
                    |d| matches!(&d.card, DropCard::Identified(a) if a.asset_id == item.asset_id),
                );
            if !known {
                self.announced.push(item);
            }
        }
    }

    /// The items to ask Steam about for drops of these games, taken from
    /// those waiting: the ones Steam said came from one of them, and the
    /// ones it didn't say.
    pub(crate) fn take_announced(&mut self, app_ids: &[u32]) -> Vec<u64> {
        let (ask, wait): (Vec<NewItem>, Vec<NewItem>) = self
            .announced
            .drain(..)
            .partition(|i| i.app_id.is_none_or(|a| app_ids.contains(&a)));
        self.announced = wait;
        ask.iter().map(|i| i.asset_id).collect()
    }

    /// Tells which card each drop found was: each game's by the cards Steam
    /// described for it, in the order announced; then by its card page's
    /// counts; the rest aren't known. Numbers each named copy. Returns the
    /// drops, named.
    pub(crate) fn identify(&mut self, found: &[Found], described: &[CardAsset]) -> Vec<Drop> {
        let mut named = Vec::new();
        for f in found {
            let (earlier, new) = self.session.drops.split_at_mut(f.first);
            let new = &mut new[..f.count];
            let cards: Vec<&CardAsset> =
                described.iter().filter(|a| a.app_id == f.app_id).collect();
            name(new, &cards, &f.before, f.after.as_deref(), earlier);
            named.extend_from_slice(new);
            if f.after.is_none() {
                self.count_in(f.app_id, &named[named.len() - f.count..]);
            }
        }
        named
    }

    /// Counts drops no look has seen into the game's set as the farmer knows
    /// it, so the next look tells its own drops by it, and numbers them.
    fn count_in(&mut self, app_id: u32, drops: &[Drop]) {
        let Some(mut game) = self.library.game(app_id).cloned() else {
            return;
        };
        for d in drops.iter().filter(|d| !d.card.is_foil()) {
            let card = d
                .card
                .name()
                .and_then(|name| game.cards.iter_mut().find(|c| c.name == name));
            if let Some(card) = card {
                card.owned += 1;
            }
        }
        self.library.update(game);
    }

    /// Makes the session's first forecast, once it has two drops farming
    /// alone to learn from.
    pub(crate) fn first_forecast(&mut self, prefs: &Preferences, now: DateTime<Utc>) {
        if self.session.first_forecast.is_some() {
            return;
        }
        let order = farm_order(&self.library, prefs, &self.set_aside);
        let made = forecast(&self.session, &self.library, &order, now);
        if !made.assumed {
            self.session.first_forecast = Some(made);
        }
    }
}

/// Names one game's new drops, in order: first by the cards Steam described
/// for it; then, when the set was known and looked at again, by its counts,
/// `after` against `before` (a count that went up, less the copies already
/// named, names a normal card); the rest aren't known. Then numbers each.
fn name(
    new: &mut [Drop],
    described: &[&CardAsset],
    before: &[Card],
    after: Option<&[Card]>,
    earlier: &[Drop],
) {
    for (drop, card) in new.iter_mut().zip(described) {
        drop.card = DropCard::Identified((*card).clone());
    }
    if let Some(after) = after.filter(|_| !before.is_empty()) {
        let mut more: Vec<(&str, u32)> = after
            .iter()
            .map(|c| {
                (
                    c.name.as_str(),
                    c.owned.saturating_sub(owned(before, &c.name)),
                )
            })
            .collect();
        for named in new.iter().filter(|d| !d.card.is_foil()) {
            let went_up = named
                .card
                .name()
                .and_then(|name| more.iter_mut().find(|(n, _)| *n == name));
            if let Some((_, up)) = went_up {
                *up = up.saturating_sub(1);
            }
        }
        let mut by_page = more
            .into_iter()
            .flat_map(|(name, up)| iter::repeat_n(name, up as usize));
        for drop in new.iter_mut().filter(|d| d.card == DropCard::Identifying) {
            let Some(name) = by_page.next() else {
                break;
            };
            drop.card = DropCard::NameOnly {
                name: name.to_owned(),
                foil: false,
            };
        }
    }
    for i in 0..new.len() {
        if new[i].card == DropCard::Identifying {
            new[i].card = DropCard::Unknown;
        }
        let (look, rest) = new.split_at_mut(i);
        rest[0].copy = copy(&rest[0], before, look, earlier);
    }
}

/// Which copy of its card `drop` made the account hold: of a normal card,
/// how many the set had before, plus one, plus the copies named before it in
/// the same look; of a foil, which the set doesn't count, how many this
/// session has dropped, plus one. `None` when it isn't known.
fn copy(drop: &Drop, before: &[Card], look: &[Drop], earlier: &[Drop]) -> Option<u32> {
    let name = drop.card.name()?;
    let foil = drop.card.is_foil();
    let same = |d: &&Drop| {
        d.app_id == drop.app_id && d.card.name() == Some(name) && d.card.is_foil() == foil
    };
    let in_look = look.iter().filter(same).count() as u32;
    if foil {
        let this_session = earlier.iter().filter(same).count() as u32;
        return Some(this_session + in_look + 1);
    }
    let held = before.iter().find(|c| c.name == name)?.owned;
    Some(held + in_look + 1)
}

/// How many of `name` the set has; none when it isn't in it.
fn owned(set: &[Card], name: &str) -> u32 {
    set.iter().find(|c| c.name == name).map_or(0, |c| c.owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at() -> DateTime<Utc> {
        DateTime::default()
    }

    fn card(name: &str, owned: u32) -> Card {
        Card {
            name: name.into(),
            owned,
        }
    }

    fn asset(asset_id: u64, app_id: u32, name: &str, foil: bool) -> CardAsset {
        CardAsset {
            asset_id,
            app_id,
            name: name.into(),
            market_hash_name: format!("{app_id}-{name}"),
            foil,
            marketable: true,
            tradable: true,
        }
    }

    fn identifying(app_id: u32, n: usize) -> Vec<Drop> {
        vec![
            Drop {
                at: at(),
                app_id,
                card: DropCard::Identifying,
                copy: None,
            };
            n
        ]
    }

    fn heavy_rain(madison: u32, scott: u32) -> Vec<Card> {
        vec![
            card("Ethan", 0),
            card("Carter", 0),
            card("Madison", madison),
            card("Norman", 0),
            card("Scott", scott),
        ]
    }

    fn named(drops: &[Drop]) -> Vec<(Option<&str>, bool, Option<u32>)> {
        drops
            .iter()
            .map(|d| (d.card.name(), d.card.is_foil(), d.copy))
            .collect()
    }

    #[test]
    fn a_card_steam_describes_is_that_card() {
        let mut new = identifying(960_910, 1);
        let madison = asset(31_002, 960_910, "Madison", false);

        name(
            &mut new,
            &[&madison],
            &heavy_rain(1, 1),
            Some(heavy_rain(2, 1).as_slice()),
            &[],
        );

        assert_eq!(new[0].card, DropCard::Identified(madison));
        assert_eq!(new[0].copy, Some(2), "a second copy: a spare");
    }

    #[test]
    fn two_copies_in_one_look_are_numbered_in_turn() {
        let mut new = identifying(960_910, 2);
        let first = asset(31_002, 960_910, "Madison", false);
        let second = asset(31_003, 960_910, "Madison", false);

        name(
            &mut new,
            &[&first, &second],
            &heavy_rain(1, 0),
            Some(heavy_rain(3, 0).as_slice()),
            &[],
        );

        assert_eq!(
            named(&new),
            [
                (Some("Madison"), false, Some(2)),
                (Some("Madison"), false, Some(3))
            ]
        );
    }

    #[test]
    fn what_steam_doesnt_say_the_card_page_does() {
        // Steam described Scott, and Madison's count went up too.
        let mut new = identifying(960_910, 2);
        let scott = asset(31_004, 960_910, "Scott", false);

        name(
            &mut new,
            &[&scott],
            &heavy_rain(1, 0),
            Some(heavy_rain(2, 1).as_slice()),
            &[],
        );

        assert_eq!(new[0].card, DropCard::Identified(scott));
        assert_eq!(
            new[1].card,
            DropCard::NameOnly {
                name: "Madison".into(),
                foil: false
            },
            "not Scott again: its count went up for the copy Steam described"
        );
        assert_eq!(named(&new)[1].2, Some(2));
    }

    #[test]
    fn a_card_nothing_tells_is_unknown() {
        // A foil the card page can't see, and no item to go by.
        let mut new = identifying(960_910, 1);
        name(
            &mut new,
            &[],
            &heavy_rain(1, 1),
            Some(heavy_rain(1, 1).as_slice()),
            &[],
        );
        assert_eq!(new[0].card, DropCard::Unknown);
        assert_eq!(new[0].copy, None);

        // A read of the badges shows no set.
        let mut read = identifying(960_910, 1);
        name(&mut read, &[], &heavy_rain(1, 1), None, &[]);
        assert_eq!(read[0].card, DropCard::Unknown);
    }

    #[test]
    fn a_set_not_known_before_names_and_numbers_nothing() {
        let mut new = identifying(960_910, 1);
        let madison = asset(31_002, 960_910, "Madison", false);
        name(
            &mut new,
            &[&madison],
            &[],
            Some(heavy_rain(2, 1).as_slice()),
            &[],
        );
        assert_eq!(
            new[0].card,
            DropCard::Identified(madison),
            "Steam still says"
        );
        assert_eq!(new[0].copy, None, "but not which copy");

        let mut unnamed = identifying(960_910, 1);
        name(
            &mut unnamed,
            &[],
            &[],
            Some(heavy_rain(2, 1).as_slice()),
            &[],
        );
        assert_eq!(unnamed[0].card, DropCard::Unknown);
    }

    #[test]
    fn foils_are_numbered_by_this_sessions_foils() {
        let thanatos = |id| asset(id, 1_145_360, "Thanatos", true);
        let earlier = vec![Drop {
            at: at(),
            app_id: 1_145_360,
            card: DropCard::Identified(thanatos(1)),
            copy: Some(1),
        }];
        let mut new = identifying(1_145_360, 2);
        let (second, third) = (thanatos(2), thanatos(3));

        let set = [card("Thanatos", 4)];
        name(&mut new, &[&second, &third], &set, Some(&set), &earlier);

        assert_eq!(
            named(&new),
            [
                (Some("Thanatos"), true, Some(2)),
                (Some("Thanatos"), true, Some(3))
            ],
            "the set's four normal ones don't count"
        );
    }

    #[test]
    fn cards_described_for_another_game_name_nothing_here() {
        let mut kept = Kept::new(at());
        kept.take(SteamLibrary::new(vec![game(1, 2, heavy_rain(1, 0))]), at());
        let (_, found) = kept.update(game(1, 1, heavy_rain(1, 1)), at());
        let found = found.expect("a drop");

        let named = kept.identify(&[found], &[asset(9, 2, "Madison", false)]);

        assert_eq!(
            named[0].card,
            DropCard::NameOnly {
                name: "Scott".into(),
                foil: false
            }
        );
        assert_eq!(named[0].copy, Some(1));
    }

    #[test]
    fn a_drop_no_look_saw_is_counted_into_the_set() {
        let mut kept = Kept::new(at());
        kept.take(SteamLibrary::new(vec![game(1, 3, heavy_rain(1, 0))]), at());
        let found = kept.take(SteamLibrary::new(vec![game(1, 2, Vec::new())]), at());
        let named = kept.identify(&found, &[asset(9, 1, "Madison", false)]);
        assert_eq!(named[0].copy, Some(2));

        let (_, found) = kept.update(game(1, 1, heavy_rain(3, 0)), at());
        let named = kept.identify(&[found.expect("a drop")], &[]);

        assert_eq!(
            (named[0].card.name(), named[0].copy),
            (Some("Madison"), Some(3)),
            "the set known before it had the copy the read found"
        );
    }

    #[test]
    fn a_page_that_is_behind_drops_nothing_twice() {
        let mut kept = Kept::new(at());
        kept.take(SteamLibrary::new(vec![game(1, 3, Vec::new())]), at());
        kept.update(game(1, 2, Vec::new()), at());

        let behind = kept.take(SteamLibrary::new(vec![game(1, 3, Vec::new())]), at());
        let (_, again) = kept.update(game(1, 2, Vec::new()), at());

        assert!(behind.is_empty() && again.is_none());
        assert_eq!(kept.session.drops.len(), 1);
    }

    #[test]
    fn items_wait_for_a_drop_of_their_game() {
        let mut kept = Kept::new(at());
        let item = |asset_id, app_id| NewItem {
            asset_id,
            app_id,
            gained_at: None,
        };
        kept.keep_announced(vec![item(1, Some(10)), item(2, Some(20)), item(3, None)]);
        kept.keep_announced(vec![item(1, Some(10))]);

        assert_eq!(
            kept.take_announced(&[10]),
            [1, 3],
            "its own, and those Steam didn't say"
        );
        assert_eq!(kept.take_announced(&[20]), [2]);
        assert!(kept.take_announced(&[10, 20]).is_empty());
    }

    fn game(app_id: u32, remaining: u32, cards: Vec<Card>) -> Game {
        Game {
            app_id,
            name: format!("Game {app_id}"),
            hours: 5.0,
            drops: library::CardDrops {
                received: 4 - remaining,
                remaining,
            },
            badge_level: 0,
            cards,
        }
    }
}
