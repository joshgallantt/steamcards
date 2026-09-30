//! The session: kept from one run of the farmer to the next, with what the
//! farmer knows beside it. It records every card that dropped, one drop per
//! copy, and then which card each was; what was played, and how; the games
//! finished; and the first forecast.
//!
//! Which card dropped is told as Steam's own site tells it (research:
//! market-and-session.md, section 2): the items Steam announced, described
//! by `IdentifyCardsUseCase`. The game's card page, looked at as its drops are
//! found, checks what Steam says: a card is a drop's only if the page's
//! count of it went up. What Steam doesn't say, the page's counts can, when
//! they can only be these drops'. Failing that, it isn't known. Which copy
//! each is comes from the account's own counts: the set's, and a foil's
//! from its foil badge.

use std::{
    collections::{HashMap, HashSet},
    iter,
    time::Duration,
};

use card::{CardAsset, CardSet, CardSets, GameCards};
use chrono::{DateTime, Utc};
use game::{AppId, Game, SteamLibrary};

use crate::{
    Drop, DropCard, Finished, Forecast, Found, Looked, Mode, NewItem, Session, SetAside, Stretch,
};

/// A session as the farmer keeps it through a run, and from one run to the
/// next: the session itself, and beside it what should outlast a pause.
pub struct KeptSession {
    pub session: Session,
    /// The library as the farmer sees it: hours counted as they're played,
    /// drops as they land.
    pub library: SteamLibrary,
    /// The card sets of the games looked at: each as last read, with the
    /// drops since counted in.
    pub sets: CardSets,
    /// Whether the library has been read yet: until it has, there's nothing
    /// to tell drops by.
    read: bool,
    /// Hours counted here while playing. The badge pages take a while to
    /// show them (about half an hour), so these are a floor under what the
    /// pages say.
    counted: HashMap<AppId, f64>,
    /// Games put behind the others after a long while without a drop.
    pub set_aside: Vec<SetAside>,
    /// Items Steam announced that nothing has asked about yet. Each waits
    /// for a drop of its game, or of any game when Steam didn't say which.
    announced: Vec<NewItem>,
    /// Games whose set, as the farmer knows it, is missing a card that
    /// dropped: one a read found that nothing could tell. Until the game's
    /// card page is read again, its counts can't tell a drop, nor number it.
    uncounted: HashSet<AppId>,
}

impl KeptSession {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self {
            session: Session {
                started_at: now,
                ..Default::default()
            },
            library: SteamLibrary::default(),
            sets: CardSets::default(),
            read: false,
            counted: HashMap::new(),
            set_aside: Vec::new(),
            announced: Vec::new(),
            uncounted: HashSet::new(),
        }
    }

    /// Takes in a fresh read of the library, keeping the hours counted here
    /// and the card sets already seen, and records the drops it shows. The
    /// session's first read is what it starts from: the drops left in the
    /// games it will farm, in `order` (the farm order of a library, less
    /// what's set aside), as the user's choices stand then.
    pub fn take(
        &mut self,
        fresh: SteamLibrary,
        order: impl Fn(&SteamLibrary, &[SetAside]) -> Vec<AppId>,
        now: DateTime<Utc>,
    ) -> Vec<Found> {
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
            let order = order(&self.library, &self.set_aside);
            let left = order
                .iter()
                .filter_map(|&id| self.library.game(id))
                .map(|g| g.drops.remaining)
                .sum();
            self.session.drops_left_at_start = Some(left);
            self.session.games_at_start = u32::try_from(order.len()).ok();
        }
        found
    }

    /// Takes in a fresh look at one game, and records the drops it shows. A
    /// page showing more drops to come than the farmer has already seen is
    /// behind, cards never undropping: it's left out, its counts and all.
    pub fn update(&mut self, looked: GameCards, now: DateTime<Utc>) -> Looked {
        let GameCards { game: fresh, set } = looked;
        let behind = self
            .library
            .game(fresh.app_id)
            .filter(|known| fresh.drops.remaining > known.drops.remaining)
            .cloned();
        if let Some(mut known) = behind {
            known.hours = known.hours.max(fresh.hours);
            self.library.update(known.clone());
            return Looked {
                game: known,
                found: None,
                set: None,
            };
        }
        let set = (!set.is_empty()).then_some(set);
        let game = self.merged(fresh);
        let found = self.record(&game, set.clone(), now);
        if let Some(set) = &set {
            self.uncounted.remove(&game.app_id);
            self.sets.update(game.app_id, set.clone());
        }
        self.library.update(game.clone());
        Looked { game, found, set }
    }

    fn merged(&self, mut fresh: Game) -> Game {
        if let Some(&hours) = self.counted.get(&fresh.app_id) {
            fresh.hours = fresh.hours.max(hours);
        }
        // Cards never undrop: a page showing more to come than the farmer has
        // already seen is behind, and would drop them twice.
        if let Some(known) = self.library.game(fresh.app_id)
            && fresh.drops.remaining > known.drops.remaining
        {
            fresh.drops = known.drops;
        }
        fresh
    }

    /// Records a drop for each card `game`'s drops left went down by since
    /// the farmer last saw it, each still being found out.
    fn record(&mut self, game: &Game, after: Option<CardSet>, now: DateTime<Utc>) -> Option<Found> {
        let known = self.library.game(game.app_id)?;
        let count = known.drops.remaining.saturating_sub(game.drops.remaining) as usize;
        if count == 0 {
            return None;
        }
        let before = if self.uncounted.contains(&game.app_id) {
            CardSet::default()
        } else {
            self.sets.set(game.app_id).cloned().unwrap_or_default()
        };
        let first = self.session.drops.len();
        self.session.drops.extend(iter::repeat_n(
            Drop {
                at: now,
                app_id: game.app_id,
                card: DropCard::Identifying,
                copy: None,
            },
            count,
        ));
        if !game.has_drops_left() {
            self.session.finished.push(Finished {
                app_id: game.app_id,
                at: now,
            });
        }
        // It drops after all.
        self.set_aside.retain(|s| s.app_id != game.app_id);
        Some(Found {
            app_id: game.app_id,
            drops: (first..first + count).collect(),
            before,
            after,
        })
    }

    /// Counts `played` towards each game's hours.
    pub fn count(&mut self, app_ids: &[AppId], played: Duration) {
        for &app_id in app_ids {
            let Some(mut game) = self.library.game(app_id).cloned() else {
                continue;
            };
            game.hours += played.as_secs_f64() / 3600.0;
            self.counted.insert(app_id, game.hours);
            self.library.update(game);
        }
    }

    pub fn name(&self, app_id: AppId) -> String {
        self.library
            .game(app_id)
            .map_or_else(|| format!("app {app_id}"), |g| g.name.clone())
    }

    /// Starts a stretch of playing these games, this way, from `now`: where
    /// it is, to stop it by.
    pub fn start(&mut self, app_ids: &[AppId], mode: Mode, now: DateTime<Utc>) -> usize {
        self.session.stretches.push(Stretch {
            app_ids: app_ids.to_vec(),
            mode,
            from: now,
            to: None,
        });
        self.session.stretches.len() - 1
    }

    /// Ends a stretch, if it goes on.
    pub fn stop(&mut self, stretch: usize, now: DateTime<Utc>) {
        if let Some(s) = self.session.stretches.get_mut(stretch) {
            s.to.get_or_insert(now);
        }
    }

    /// Puts a game behind the others once more: how often it has been.
    pub fn set_aside(&mut self, app_id: AppId, now: DateTime<Utc>) -> u8 {
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
    /// One Steam says arrived before the session began isn't one of its
    /// drops (research §2.2).
    pub fn keep_announced(&mut self, items: Vec<NewItem>) {
        let began = self.session.started_at.timestamp();
        for item in items {
            let earlier = item.gained_at.is_some_and(|at| at.timestamp() < began);
            let known = self.announced.iter().any(|i| i.asset_id == item.asset_id)
                || self.session.drops.iter().any(
                    |d| matches!(&d.card, DropCard::Identified(a) if a.asset_id == item.asset_id),
                );
            if !earlier && !known {
                self.announced.push(item);
            }
        }
    }

    /// The items to ask Steam about for drops of these games, taken from
    /// those waiting: the ones Steam said came from one of them, and the
    /// ones it didn't say.
    pub fn take_announced(&mut self, app_ids: &[AppId]) -> Vec<NewItem> {
        let (ask, wait): (Vec<NewItem>, Vec<NewItem>) = self
            .announced
            .drain(..)
            .partition(|i| i.app_id.is_none_or(|a| app_ids.contains(&a)));
        self.announced = wait;
        ask
    }

    /// Whether items wait that may name a drop of this game that only its
    /// card page could: worth asking Steam about now, without a new drop.
    pub fn has_items_for(&self, app_id: AppId) -> bool {
        let named_by_page = self
            .session
            .drops
            .iter()
            .any(|d| d.app_id == app_id && matches!(d.card, DropCard::NameOnly { .. }));
        named_by_page
            && self
                .announced
                .iter()
                .any(|i| i.app_id.is_none_or(|a| a == app_id))
    }

    /// Tells which card each drop found was, and which copy: by the cards
    /// Steam described, where the game's card page shows them; then by the
    /// page's counts, where they can only be these drops'. The rest aren't
    /// known. A card Steam described that none of them is may be a drop the
    /// page named before, which it then names; else it's no drop of this
    /// session's, and is left out. Returns where the drops are, to tell of
    /// them.
    pub fn identify(&mut self, found: &[Found], described: &[CardAsset]) -> Vec<usize> {
        let mut left: Vec<&CardAsset> = described.iter().collect();
        let mut told = Vec::new();
        for f in found {
            let (own, others): (Vec<&CardAsset>, Vec<&CardAsset>) =
                left.into_iter().partition(|a| a.app_id == f.app_id);
            let unused = self.name_drops(f, &own);
            left = others.into_iter().chain(unused).collect();
            if f.after.is_none() {
                self.count_in(f.app_id, &f.drops);
            }
            told.extend_from_slice(&f.drops);
        }
        self.name_earlier(&left, &told);
        told
    }

    /// Names one game's new drops, and numbers them. Returns the cards Steam
    /// described that none of them is.
    fn name_drops<'a>(&mut self, f: &Found, described: &[&'a CardAsset]) -> Vec<&'a CardAsset> {
        // How far each card's count went up, when the set before is known.
        let mut up = f
            .after
            .as_ref()
            .filter(|_| !f.before.is_empty())
            .map(|after| risen(&f.before, after));
        let mut unused = Vec::new();
        for &card in described {
            let unnamed = self.unnamed(f);
            let Some(&at) = unnamed.first() else {
                unused.push(card);
                continue;
            };
            let shown = match &mut up {
                // A normal card's count went up for it; a foil takes a drop
                // the counts leave over.
                Some(up) if card.foil => unnamed.len() > total(up),
                Some(up) => take_one(up, &card.name),
                // Without the set before, the page can only say a normal
                // card is held at all.
                None => card.foil || f.after.as_ref().is_none_or(|a| a.owned(&card.name) > 0),
            };
            if shown {
                self.session.drops[at].card = DropCard::Identified(card.clone());
            } else {
                unused.push(card);
            }
        }
        // The page's counts name the rest, when every card that went up can
        // only be one of these drops: it went up no more often than they
        // number. More, and it went up for something else too.
        if let Some(up) = up {
            let unnamed = self.unnamed(f);
            if total(&up) <= unnamed.len() {
                let names = up
                    .iter()
                    .flat_map(|&(name, n)| iter::repeat_n(name, n as usize));
                for (i, name) in unnamed.into_iter().zip(names) {
                    self.session.drops[i].card = DropCard::NameOnly {
                        name: name.to_owned(),
                        foil: false,
                    };
                }
            }
        }
        for i in self.unnamed(f) {
            self.session.drops[i].card = DropCard::Unknown;
        }
        for k in 0..f.drops.len() {
            self.session.drops[f.drops[k]].copy = copy(&self.session.drops, f, k);
        }
        unused
    }

    /// Where `f`'s drops not named yet are.
    fn unnamed(&self, f: &Found) -> Vec<usize> {
        f.drops
            .iter()
            .copied()
            .filter(|&i| self.session.drops[i].card == DropCard::Identifying)
            .collect()
    }

    /// Counts drops no look has seen into the game's set as the farmer knows
    /// it, so the next look tells its own drops by it. A drop whose card
    /// isn't known leaves the set missing a card until it's read again.
    fn count_in(&mut self, app_id: AppId, drops: &[usize]) {
        let Some(mut set) = self.sets.set(app_id).cloned() else {
            return;
        };
        if set.is_empty() {
            return;
        }
        for &i in drops {
            let card = &self.session.drops[i].card;
            if card.is_foil() {
                continue;
            }
            if !card.name().is_some_and(|name| set.count_in(name)) {
                self.uncounted.insert(app_id);
            }
        }
        self.sets.update(app_id, set);
    }

    /// Names a drop only its card page could name by the item Steam
    /// described for it since: the same card, now with its copy's item.
    fn name_earlier(&mut self, cards: &[&CardAsset], told: &[usize]) {
        for &card in cards {
            let page_named = DropCard::NameOnly {
                name: card.name.clone(),
                foil: card.foil,
            };
            let earlier = self.session.drops.iter_mut().enumerate().find(|(i, d)| {
                !told.contains(i) && d.app_id == card.app_id && d.card == page_named
            });
            if let Some((_, drop)) = earlier {
                drop.card = DropCard::Identified(card.clone());
            }
        }
    }

    /// The games with foils among these drops: which copy each is, their
    /// foil badges say.
    pub fn foil_games(&self, told: &[usize]) -> Vec<AppId> {
        let mut games = Vec::new();
        for drop in told.iter().filter_map(|&i| self.session.drops.get(i)) {
            if drop.card.is_foil() && !games.contains(&drop.app_id) {
                games.push(drop.app_id);
            }
        }
        games
    }

    /// Numbers a game's foils among these drops by its foil badge's counts,
    /// after them: how many of that foil the account has, less the copies of
    /// it found after this one.
    pub fn number_foils(&mut self, app_id: AppId, told: &[usize], foils: &CardSet) {
        let theirs: Vec<usize> = told
            .iter()
            .copied()
            .filter(|&i| {
                let d = &self.session.drops[i];
                d.app_id == app_id && d.card.is_foil()
            })
            .collect();
        for (k, &i) in theirs.iter().enumerate() {
            let Some(name) = self.session.drops[i].card.name().map(str::to_owned) else {
                continue;
            };
            let later = theirs[k + 1..]
                .iter()
                .filter(|&&j| self.session.drops[j].card.name() == Some(name.as_str()))
                .count();
            self.session.drops[i].copy = foils
                .owned(&name)
                .checked_sub(u32::try_from(later).unwrap_or(u32::MAX))
                .filter(|&n| n > 0);
        }
    }

    /// Makes the session's first forecast, once it has two drops farming
    /// alone to learn from, over the games farmed in `order`.
    pub fn first_forecast(
        &mut self,
        order: impl Fn(&SteamLibrary, &[SetAside]) -> Vec<AppId>,
        now: DateTime<Utc>,
    ) {
        if self.session.first_forecast.is_some() {
            return;
        }
        let order = order(&self.library, &self.set_aside);
        let made = Forecast::of(&self.session, &self.library, &order, now);
        if !made.assumed {
            self.session.first_forecast = Some(made);
        }
    }
}

/// Which copy of its card the `k`th of `f`'s drops made the account hold.
/// Of a normal card: how many the set had before, plus one, plus the copies
/// of it before this one in the same look. Without the set before, how many
/// the set has after, less the copies after this one, when every drop of
/// the look was named, so none of them can be another. A foil's comes from
/// its foil badge, after. `None` while it isn't known.
fn copy(drops: &[Drop], f: &Found, k: usize) -> Option<u32> {
    let card = &drops[f.drops[k]].card;
    let name = card.name()?;
    if card.is_foil() {
        return None;
    }
    let copies = |of: &[usize]| {
        let n = of
            .iter()
            .filter(|&&i| drops[i].card.name() == Some(name) && !drops[i].card.is_foil())
            .count();
        u32::try_from(n).unwrap_or(u32::MAX)
    };
    if !f.before.is_empty() {
        let held = f.before.cards().iter().find(|c| c.name == name)?.owned;
        return Some(held + copies(&f.drops[..k]) + 1);
    }
    let all_named = f.drops.iter().all(|&i| drops[i].card.name().is_some());
    let after = f.after.as_ref().filter(|_| all_named)?;
    let held = after.cards().iter().find(|c| c.name == name)?.owned;
    held.checked_sub(copies(&f.drops[k + 1..]))
        .filter(|&n| n > 0)
}

/// How far each card's count went up from `before` to `after`, in the set's
/// order.
fn risen<'a>(before: &CardSet, after: &'a CardSet) -> Vec<(&'a str, u32)> {
    after
        .cards()
        .iter()
        .map(|c| {
            (
                c.name.as_str(),
                c.owned.saturating_sub(before.owned(&c.name)),
            )
        })
        .collect()
}

/// How many went up, in all.
fn total(up: &[(&str, u32)]) -> usize {
    up.iter().map(|&(_, n)| n as usize).sum()
}

/// Takes one of `name`'s going up as a drop's: whether it went up.
fn take_one(up: &mut [(&str, u32)], name: &str) -> bool {
    match up.iter_mut().find(|(n, _)| *n == name) {
        Some((_, n)) if *n > 0 => {
            *n -= 1;
            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use std::slice;

    use card::{AssetId, Card};
    use game::CardDrops;

    use super::*;

    const HEAVY_RAIN: AppId = AppId(960_910);

    fn at() -> DateTime<Utc> {
        DateTime::default()
    }

    fn card(name: &str, owned: u32) -> Card {
        Card {
            name: name.into(),
            owned,
        }
    }

    fn asset(asset_id: u64, app_id: AppId, name: &str, foil: bool) -> CardAsset {
        CardAsset {
            asset_id: AssetId(asset_id),
            app_id,
            name: name.into(),
            market_hash_name: format!("{app_id}-{name}"),
            foil,
            marketable: true,
            tradable: true,
        }
    }

    /// Heavy Rain's set: how many of each card the account has.
    fn heavy_rain_with(ethan: u32, madison: u32, norman: u32, scott: u32) -> CardSet {
        CardSet::new(vec![
            card("Ethan", ethan),
            card("Carter", 0),
            card("Madison", madison),
            card("Norman", norman),
            card("Scott", scott),
        ])
    }

    fn heavy_rain(madison: u32, scott: u32) -> CardSet {
        heavy_rain_with(0, madison, 0, scott)
    }

    fn game(app_id: AppId, remaining: u32) -> Game {
        Game {
            app_id,
            name: format!("Game {app_id}"),
            hours: 5.0,
            drops: CardDrops {
                received: 4 - remaining,
                remaining,
            },
            badge_level: 0,
        }
    }

    /// A look at a game's card page: `remaining` to come, and its set.
    fn page(app_id: AppId, remaining: u32, set: CardSet) -> GameCards {
        GameCards {
            game: game(app_id, remaining),
            set,
        }
    }

    /// Every game with drops left, in the library's order: the farm order,
    /// as far as these tests go.
    fn every(library: &SteamLibrary, _: &[SetAside]) -> Vec<AppId> {
        library.with_drops_left().map(|g| g.app_id).collect()
    }

    /// A session that has read the library, and looked at Heavy Rain: `left`
    /// drops to come, and its set.
    fn farming(left: u32, set: CardSet) -> KeptSession {
        let mut kept = KeptSession::new(at());
        kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, left)]), every, at());
        kept.update(page(HEAVY_RAIN, left, set), at());
        kept
    }

    /// What each drop is, and which copy.
    fn named(kept: &KeptSession, told: &[usize]) -> Vec<(Option<String>, bool, Option<u32>)> {
        told.iter()
            .map(|&i| {
                let d = &kept.session.drops[i];
                (d.card.name().map(str::to_owned), d.card.is_foil(), d.copy)
            })
            .collect()
    }

    fn some(name: &str) -> Option<String> {
        Some(name.to_owned())
    }

    /// A look at Heavy Rain: `left` to come, and its set now.
    fn looks(kept: &mut KeptSession, left: u32, set: CardSet) -> Option<Found> {
        kept.update(page(HEAVY_RAIN, left, set), at()).found
    }

    #[test]
    fn a_card_steam_describes_is_that_card() {
        let mut kept = farming(3, heavy_rain(1, 1));
        let found = looks(&mut kept, 2, heavy_rain(2, 1)).expect("a drop");
        let madison = asset(31_002, HEAVY_RAIN, "Madison", false);

        let told = kept.identify(&[found], slice::from_ref(&madison));

        assert_eq!(
            kept.session.drops[told[0]].card,
            DropCard::Identified(madison)
        );
        assert_eq!(
            kept.session.drops[told[0]].copy,
            Some(2),
            "a second copy: a spare"
        );
    }

    #[test]
    fn two_copies_in_one_look_are_numbered_in_turn() {
        let mut kept = farming(3, heavy_rain(1, 0));
        let found = looks(&mut kept, 1, heavy_rain(3, 0)).expect("drops");
        let first = asset(31_002, HEAVY_RAIN, "Madison", false);
        let second = asset(31_003, HEAVY_RAIN, "Madison", false);

        let told = kept.identify(&[found], &[first, second]);

        assert_eq!(
            named(&kept, &told),
            [
                (some("Madison"), false, Some(2)),
                (some("Madison"), false, Some(3))
            ]
        );
    }

    #[test]
    fn what_steam_doesnt_say_the_card_page_does() {
        // Steam described Scott, and Madison's count went up too.
        let mut kept = farming(3, heavy_rain(1, 0));
        let found = looks(&mut kept, 1, heavy_rain(2, 1)).expect("drops");
        let scott = asset(31_004, HEAVY_RAIN, "Scott", false);

        let told = kept.identify(&[found], slice::from_ref(&scott));

        assert_eq!(
            kept.session.drops[told[0]].card,
            DropCard::Identified(scott)
        );
        assert_eq!(
            kept.session.drops[told[1]].card,
            DropCard::NameOnly {
                name: "Madison".into(),
                foil: false
            },
            "not Scott again: its count went up for the copy Steam described"
        );
        assert_eq!(named(&kept, &told)[1].2, Some(2));
    }

    #[test]
    fn a_card_steam_describes_whose_count_didnt_go_up_isnt_the_drop() {
        // An item left over from before: Steam describes Scott, but the
        // page shows Ethan went up.
        let mut kept = farming(3, heavy_rain(1, 1));
        let found = looks(&mut kept, 2, heavy_rain_with(1, 1, 0, 1)).expect("a drop");

        let told = kept.identify(&[found], &[asset(31_004, HEAVY_RAIN, "Scott", false)]);

        assert_eq!(
            named(&kept, &told),
            [(some("Ethan"), false, Some(1))],
            "the page's count names it, and it's a first copy"
        );
    }

    #[test]
    fn a_foil_takes_a_drop_the_counts_leave_over() {
        let mut kept = farming(3, heavy_rain(1, 1));
        let foil = asset(31_005, HEAVY_RAIN, "Madison", true);

        let one_normal = looks(&mut kept, 2, heavy_rain(2, 1)).expect("a drop");
        let told = kept.identify(&[one_normal], slice::from_ref(&foil));
        assert_eq!(
            named(&kept, &told),
            [(some("Madison"), false, Some(2))],
            "Madison's count went up: the drop was a normal Madison"
        );

        let a_foil = looks(&mut kept, 1, heavy_rain(2, 1)).expect("a drop");
        let told = kept.identify(&[a_foil], slice::from_ref(&foil));
        assert_eq!(kept.session.drops[told[0]].card, DropCard::Identified(foil));
        assert_eq!(
            kept.session.drops[told[0]].copy, None,
            "its foil badge says which copy"
        );
    }

    #[test]
    fn a_card_nothing_tells_is_unknown() {
        // A foil the card page can't see, and no item to go by.
        let mut kept = farming(3, heavy_rain(1, 1));
        let found = looks(&mut kept, 2, heavy_rain(1, 1)).expect("a drop");

        let told = kept.identify(&[found], &[]);

        assert_eq!(kept.session.drops[told[0]].card, DropCard::Unknown);
        assert_eq!(kept.session.drops[told[0]].copy, None);
    }

    #[test]
    fn a_read_is_told_by_a_look_at_the_game_after_it() {
        let mut kept = farming(3, heavy_rain(1, 1));
        let mut found = kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 2)]), every, at());
        assert!(found[0].needs_look(), "the badges show no sets");

        let looked = kept.update(page(HEAVY_RAIN, 2, heavy_rain(2, 1)), at());
        found[0].join(looked);
        let told = kept.identify(&found, &[]);

        assert_eq!(named(&kept, &told), [(some("Madison"), false, Some(2))]);
        assert_eq!(
            kept.sets.set(HEAVY_RAIN),
            Some(&heavy_rain(2, 1)),
            "the set, as the look found it"
        );
    }

    #[test]
    fn counts_that_went_up_for_a_drop_nothing_told_name_no_later_one() {
        // A read found Madison's drop, and nothing could tell it: the look
        // after it failed, and Steam gave only a count.
        let mut kept = farming(3, heavy_rain(1, 1));
        let read = kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 2)]), every, at());
        let told = kept.identify(&read, &[]);
        assert_eq!(named(&kept, &told), [(None, false, None)]);

        // Then Scott drops: the page's counts went up for both.
        let found = looks(&mut kept, 1, heavy_rain(2, 2)).expect("a drop");
        let told = kept.identify(&[found], &[]);
        assert_eq!(
            named(&kept, &told),
            [(None, false, None)],
            "not Madison, nor a 2nd copy of anything: it can't be told"
        );

        // With the set read again, the next is told.
        let found = looks(&mut kept, 0, heavy_rain_with(0, 2, 1, 2)).expect("a drop");
        let told = kept.identify(&[found], &[]);
        assert_eq!(named(&kept, &told), [(some("Norman"), false, Some(1))]);
    }

    #[test]
    fn a_set_not_known_before_is_numbered_from_the_look_after() {
        let mut kept = KeptSession::new(at());
        kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 3)]), every, at());
        let mut found = kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 2)]), every, at());
        found[0].join(kept.update(page(HEAVY_RAIN, 2, heavy_rain(2, 1)), at()));
        let madison = asset(31_002, HEAVY_RAIN, "Madison", false);

        let told = kept.identify(&found, slice::from_ref(&madison));

        assert_eq!(
            kept.session.drops[told[0]].card,
            DropCard::Identified(madison)
        );
        assert_eq!(
            kept.session.drops[told[0]].copy,
            Some(2),
            "the page after shows two Madisons: this made the second"
        );
    }

    #[test]
    fn without_a_set_nothing_says_which_copy() {
        // Its set was never read, and the look after the read failed.
        let mut kept = KeptSession::new(at());
        kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 3)]), every, at());
        let found = kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 2)]), every, at());
        let madison = asset(31_002, HEAVY_RAIN, "Madison", false);

        let told = kept.identify(&found, slice::from_ref(&madison));

        assert_eq!(
            kept.session.drops[told[0]].card,
            DropCard::Identified(madison),
            "Steam still says which card"
        );
        assert_eq!(kept.session.drops[told[0]].copy, None, "but not which copy");
    }

    #[test]
    fn foils_are_numbered_by_their_badge() {
        let thanatos = |id| asset(id, HEAVY_RAIN, "Thanatos", true);
        let mut kept = farming(3, heavy_rain(1, 1));
        let found = looks(&mut kept, 1, heavy_rain(1, 1)).expect("drops");
        let told = kept.identify(&[found], &[thanatos(2), thanatos(3)]);
        assert_eq!(kept.foil_games(&told), [HEAVY_RAIN]);

        kept.number_foils(HEAVY_RAIN, &told, &CardSet::new(vec![card("Thanatos", 4)]));

        assert_eq!(
            named(&kept, &told),
            [
                (some("Thanatos"), true, Some(3)),
                (some("Thanatos"), true, Some(4))
            ],
            "two held before: these made the 3rd and 4th"
        );
    }

    #[test]
    fn cards_described_for_another_game_name_nothing_here() {
        let mut kept = farming(3, heavy_rain(1, 0));
        let found = looks(&mut kept, 2, heavy_rain(1, 1)).expect("a drop");

        let told = kept.identify(&[found], &[asset(9, AppId(2), "Madison", false)]);

        assert_eq!(named(&kept, &told), [(some("Scott"), false, Some(1))]);
    }

    #[test]
    fn a_drop_no_look_saw_is_counted_into_the_set() {
        let mut kept = farming(3, heavy_rain(1, 0));
        let found = kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 2)]), every, at());
        let told = kept.identify(&found, &[asset(9, HEAVY_RAIN, "Madison", false)]);
        assert_eq!(named(&kept, &told), [(some("Madison"), false, Some(2))]);

        let found = looks(&mut kept, 1, heavy_rain(3, 0)).expect("a drop");
        let told = kept.identify(&[found], &[]);

        assert_eq!(
            named(&kept, &told),
            [(some("Madison"), false, Some(3))],
            "the set known before it had the copy the read found"
        );
    }

    #[test]
    fn a_page_that_is_behind_changes_nothing() {
        let mut kept = farming(3, heavy_rain(1, 1));
        let found = looks(&mut kept, 2, heavy_rain(2, 1)).expect("a drop");
        kept.identify(&[found], &[]);
        let behind = kept.take(SteamLibrary::new(vec![game(HEAVY_RAIN, 3)]), every, at());
        let again = looks(&mut kept, 3, heavy_rain(1, 1));
        assert!(
            behind.is_empty() && again.is_none(),
            "nothing dropped twice"
        );
        assert_eq!(
            kept.sets.set(HEAVY_RAIN),
            Some(&heavy_rain(2, 1)),
            "nor its counts taken back"
        );

        let found = looks(&mut kept, 1, heavy_rain(2, 2)).expect("a drop");
        let told = kept.identify(&[found], &[]);
        assert_eq!(
            named(&kept, &told),
            [(some("Scott"), false, Some(2))],
            "Scott, not Madison a second time"
        );
        assert_eq!(kept.session.drops.len(), 2);
    }

    #[test]
    fn an_item_described_late_names_the_drop_the_page_named() {
        let mut kept = farming(3, heavy_rain(1, 1));
        let found = looks(&mut kept, 1, heavy_rain(2, 2)).expect("drops");
        let madison = asset(30_001, HEAVY_RAIN, "Madison", false);
        let first = kept.identify(&[found], slice::from_ref(&madison));
        assert_eq!(
            kept.session.drops[first[1]].card,
            DropCard::NameOnly {
                name: "Scott".into(),
                foil: false
            }
        );

        let scott = asset(30_002, HEAVY_RAIN, "Scott", false);
        let ethan = asset(30_003, HEAVY_RAIN, "Ethan", false);
        let found = looks(&mut kept, 0, heavy_rain_with(1, 2, 0, 2)).expect("a drop");
        let told = kept.identify(&[found], &[scott.clone(), ethan.clone()]);

        assert_eq!(
            kept.session.drops[told[0]].card,
            DropCard::Identified(ethan)
        );
        assert_eq!(kept.session.drops[told[0]].copy, Some(1));
        assert_eq!(
            kept.session.drops[first[1]].card,
            DropCard::Identified(scott),
            "Scott's own item, for the drop the page named"
        );
        assert_eq!(kept.session.drops[first[1]].copy, Some(2), "still the 2nd");
    }

    #[test]
    fn items_wait_for_a_drop_of_their_game() {
        let mut kept = KeptSession::new(at());
        let item = |asset_id, app_id: Option<u32>| NewItem {
            asset_id: AssetId(asset_id),
            app_id: app_id.map(AppId),
            gained_at: None,
        };
        kept.keep_announced(vec![item(1, Some(10)), item(2, Some(20)), item(3, None)]);
        kept.keep_announced(vec![item(1, Some(10))]);

        let ids = |items: Vec<NewItem>| items.iter().map(|i| i.asset_id.0).collect::<Vec<_>>();
        assert_eq!(
            ids(kept.take_announced(&[AppId(10)])),
            [1, 3],
            "its own, and those Steam didn't say"
        );
        assert_eq!(ids(kept.take_announced(&[AppId(20)])), [2]);
        assert!(kept.take_announced(&[AppId(10), AppId(20)]).is_empty());
    }

    #[test]
    fn an_item_from_before_the_session_is_none_of_its_drops() {
        let started = DateTime::from_timestamp(1_790_700_000, 0).unwrap();
        let mut kept = KeptSession::new(started);
        let item = |asset_id, gained: i64| NewItem {
            asset_id: AssetId(asset_id),
            app_id: Some(AppId(10)),
            gained_at: DateTime::from_timestamp(gained, 0),
        };

        kept.keep_announced(vec![item(1, 1_790_600_000), item(2, 1_790_700_000)]);

        assert_eq!(
            kept.take_announced(&[AppId(10)])
                .iter()
                .map(|i| i.asset_id.0)
                .collect::<Vec<_>>(),
            [2]
        );
    }
}
