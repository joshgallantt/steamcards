use std::{
    collections::{BTreeMap, VecDeque},
    sync::Mutex,
    time::Duration,
};

use async_trait::async_trait;
use card::{AssetId, Card, CardAsset, CardKind, CardRepository, CardSet, GameCards, NewItem};
use chrono::{DateTime, Utc};
use game::{
    AppId, CardDrops, Game, GameRepository, Playing, PlayingRepository, PlayingSignal, SteamLibrary,
};
use tokio::{sync::mpsc, time::Instant};

/// A game's card drops, as the stand-in plays them out.
#[derive(Debug, Clone)]
struct Farmed {
    game: Game,
    /// Its set, and how many of each card the account has: only its card
    /// page shows it.
    cards: Vec<Card>,
    /// Played time from one card to the next; `None` never drops.
    drop_every: Option<Duration>,
    /// Hours it needs before its cards drop at all.
    needs_hours: f64,
    /// Played time since the last card dropped.
    since_drop: Duration,
    /// The cards that drop next, in order, and of which kind. Once they've
    /// dropped, a normal card named after how many have.
    next: VecDeque<(String, CardKind)>,
    /// Its set in foil, and how many of each the account has: only its foil
    /// badge's page shows them.
    foils: Vec<Card>,
}

struct State {
    games: BTreeMap<AppId, Farmed>,
    /// Cards drop only for a game played alone.
    alone_only: bool,
    playing: Vec<AppId>,
    /// Up to when what's playing has been counted.
    counted_to: Instant,
    /// What another device is playing, if anything, when Steam says.
    blocked: Option<Option<AppId>>,
    signed_on: bool,
    /// How often a session signed on.
    sign_ons: usize,
    plays: Vec<(Vec<AppId>, bool)>,
    stops: usize,
    reads: usize,
    down: bool,
    /// Another session took this one's place: what's asked of Steam fails,
    /// until a session signs on here again.
    replaced: bool,
    /// Every copy of a card that has dropped, as Steam describes it.
    held: Vec<CardAsset>,
    /// Those Steam hasn't announced yet.
    unannounced: Vec<NewItem>,
    /// Every ask to describe items: the asset IDs asked about.
    describes: Vec<Vec<AssetId>>,
    cant_describe: bool,
    /// How long Steam takes to say which cards items are.
    describe_takes: Duration,
}

/// Steam, as far as farming sees it, in memory: a library whose badges answer
/// as its games have been played, on tokio's clock, so a test can run hours
/// of farming on paused time. Each card that drops is an item of its own,
/// which Steam announces when told to, and describes when asked. Whether
/// another device is playing is said only to a session signed on, and while
/// it plays, nothing is played here. It's every repository farming plays and
/// looks through: the game component's, for the library and playing, and
/// the card component's.
pub struct FakeSteamAccount {
    state: Mutex<State>,
    said: mpsc::UnboundedSender<PlayingSignal>,
    to_hear: tokio::sync::Mutex<mpsc::UnboundedReceiver<PlayingSignal>>,
    announced: mpsc::UnboundedSender<Vec<NewItem>>,
    to_announce: tokio::sync::Mutex<mpsc::UnboundedReceiver<Vec<NewItem>>>,
}

impl Default for FakeSteamAccount {
    fn default() -> Self {
        Self::new()
    }
}

/// Where the stand-in's asset IDs start.
const FIRST_ASSET: u64 = 30_001;

impl FakeSteamAccount {
    pub fn new() -> Self {
        let (said, to_hear) = mpsc::unbounded_channel();
        let (announced, to_announce) = mpsc::unbounded_channel();
        Self {
            state: Mutex::new(State {
                games: BTreeMap::new(),
                alone_only: false,
                playing: Vec::new(),
                counted_to: Instant::now(),
                blocked: None,
                signed_on: false,
                sign_ons: 0,
                plays: Vec::new(),
                stops: 0,
                reads: 0,
                down: false,
                replaced: false,
                held: Vec::new(),
                unannounced: Vec::new(),
                describes: Vec::new(),
                cant_describe: false,
                describe_takes: Duration::ZERO,
            }),
            said,
            to_hear: tokio::sync::Mutex::new(to_hear),
            announced,
            to_announce: tokio::sync::Mutex::new(to_announce),
        }
    }

    /// Adds "Game <app ID>", with `hours` played and `cards` drops to come:
    /// one every `every` of play once it has 3 hours (`None`: never).
    pub fn add_game(&self, app_id: u32, hours: f64, cards: u32, every: Option<Duration>) {
        self.state.lock().unwrap().games.insert(
            AppId(app_id),
            Farmed {
                game: Game {
                    app_id: AppId(app_id),
                    name: format!("Game {app_id}"),
                    hours,
                    drops: CardDrops {
                        received: 0,
                        remaining: cards,
                    },
                    badge_level: 0,
                    private: false,
                    bought_at: None,
                },
                cards: Vec::new(),
                drop_every: every,
                needs_hours: 3.0,
                since_drop: Duration::ZERO,
                next: VecDeque::new(),
                foils: Vec::new(),
            },
        );
    }

    /// Names a game, as its badge and card page do.
    pub fn name(&self, app_id: u32, name: &str) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.game.name = name.to_owned();
        }
    }

    /// Gives a game its set of cards, with how many of each the account
    /// has. Only its card page shows it; the badge pages don't.
    pub fn set(&self, app_id: u32, cards: &[(&str, u32)]) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.cards = cards
                .iter()
                .map(|&(name, owned)| Card {
                    name: name.to_owned(),
                    owned,
                })
                .collect();
        }
    }

    /// The foils of a game the account holds already: how many of each.
    pub fn holds_foils(&self, app_id: u32, foils: &[(&str, u32)]) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.foils = foils
                .iter()
                .map(|&(name, owned)| Card {
                    name: name.to_owned(),
                    owned,
                })
                .collect();
        }
    }

    /// The next cards to drop for a game, in order: normal ones.
    pub fn will_drop(&self, app_id: u32, names: &[&str]) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.next.extend(
                names
                    .iter()
                    .map(|name| ((*name).to_owned(), CardKind::Normal)),
            );
        }
    }

    /// The next card to drop for a game is a foil.
    pub fn will_drop_foil(&self, app_id: u32, name: &str) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.next.push_back((name.to_owned(), CardKind::Foil));
        }
    }

    /// A card drops for `app_id` now, whatever is played here: as when
    /// another device plays it.
    pub fn drops_now(&self, app_id: u32) {
        self.settle();
        let mut s = self.state.lock().unwrap();
        drop_card(&mut s, AppId(app_id));
    }

    /// Cards drop only for a game played on its own.
    pub fn drops_only_alone(&self) {
        self.state.lock().unwrap().alone_only = true;
    }

    /// A game's cards drop from its first minute of play, as on accounts
    /// Steam doesn't hold back for 3 hours.
    pub fn drops_straight_away(&self, app_id: u32) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.needs_hours = 0.0;
        }
    }

    /// The account marks a game private: the badge pages still list it, and
    /// Steam drops no cards for it.
    pub fn private(&self, app_id: u32) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.game.private = true;
        }
    }

    /// The account bought a game `at`, so lately that Steam may still refund
    /// it.
    pub fn bought(&self, app_id: u32, at: DateTime<Utc>) {
        if let Some(f) = self.state.lock().unwrap().games.get_mut(&AppId(app_id)) {
            f.game.bought_at = Some(at);
        }
    }

    /// Another device starts playing `app_id` on the account.
    pub fn block(&self, app_id: Option<u32>) {
        self.settle();
        let app_id = app_id.map(AppId);
        let mut s = self.state.lock().unwrap();
        s.blocked = Some(app_id);
        if s.signed_on {
            let _ = self.said.send(PlayingSignal::Blocked(app_id));
        }
    }

    /// It stops.
    pub fn unblock(&self) {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.blocked = None;
        if s.signed_on {
            let _ = self.said.send(PlayingSignal::Unblocked);
        }
    }

    /// Another device takes over playing, and Steam signs this session off
    /// to let it. Its game starts when the test says, with `block`.
    pub fn take_over(&self) {
        self.sign_off(PlayingSignal::TakenOver);
    }

    /// Steam says new items arrived, giving only a count.
    pub fn new_items(&self) {
        let _ = self.announced.send(Vec::new());
    }

    /// Steam says which items arrived since it last said: each card that
    /// dropped, with its game.
    pub fn announce(&self) {
        self.settle();
        let items = std::mem::take(&mut self.state.lock().unwrap().unannounced);
        let _ = self.announced.send(items);
    }

    /// Steam announces these items.
    pub fn announce_items(&self, items: Vec<NewItem>) {
        let _ = self.announced.send(items);
    }

    /// Asking which cards items are fails from now on.
    pub fn cant_describe(&self) {
        self.state.lock().unwrap().cant_describe = true;
    }

    /// Steam takes this long to say which cards items are, as when it's
    /// slow to answer.
    pub fn describes_after(&self, takes: Duration) {
        self.state.lock().unwrap().describe_takes = takes;
    }

    /// Every ask to describe items: the asset IDs asked about.
    pub fn describes(&self) -> Vec<Vec<u64>> {
        self.state
            .lock()
            .unwrap()
            .describes
            .iter()
            .map(|asked| asked.iter().map(|id| id.0).collect())
            .collect()
    }

    /// Every copy of a card that has dropped, as Steam describes it.
    pub fn held(&self) -> Vec<CardAsset> {
        self.settle();
        self.state.lock().unwrap().held.clone()
    }

    /// Another session signs on in this one's place: Steam signs this one
    /// off, and says why.
    pub fn replace(&self) {
        self.sign_off(PlayingSignal::Replaced);
        self.state.lock().unwrap().replaced = true;
    }

    /// The connection to Steam goes.
    pub fn lose(&self, why: &str) {
        self.sign_off(PlayingSignal::Lost(why.into()));
    }

    /// How often a session signed on.
    pub fn sign_ons(&self) -> usize {
        self.state.lock().unwrap().sign_ons
    }

    /// The session is signed off, and told so: nothing it played counts.
    fn sign_off(&self, why: PlayingSignal) {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.signed_on = false;
        s.playing.clear();
        let _ = self.said.send(why);
    }

    /// The badges can't be read, as if steamcommunity.com were down, or can
    /// again.
    pub fn go_down(&self, down: bool) {
        self.state.lock().unwrap().down = down;
    }

    /// Every list of games played, and whether online, in order.
    pub fn plays(&self) -> Vec<(Vec<u32>, bool)> {
        self.state
            .lock()
            .unwrap()
            .plays
            .iter()
            .map(|(games, online)| (games.iter().map(|g| g.0).collect(), *online))
            .collect()
    }

    /// Just the games of each play.
    pub fn played(&self) -> Vec<Vec<u32>> {
        self.plays().into_iter().map(|(games, _)| games).collect()
    }

    /// What's playing now.
    pub fn playing_now(&self) -> Vec<u32> {
        self.state
            .lock()
            .unwrap()
            .playing
            .iter()
            .map(|g| g.0)
            .collect()
    }

    pub fn stops(&self) -> usize {
        self.state.lock().unwrap().stops
    }

    /// How often the whole library was read.
    pub fn reads(&self) -> usize {
        self.state.lock().unwrap().reads
    }

    /// A game as it stands now.
    pub fn game(&self, app_id: u32) -> Option<Game> {
        self.settle();
        self.state
            .lock()
            .unwrap()
            .games
            .get(&AppId(app_id))
            .map(|f| f.game.clone())
    }

    /// Counts the time since last counted for what's playing: hours for each
    /// game, and the cards that drop.
    fn settle(&self) {
        let mut guard = self.state.lock().unwrap();
        let s = &mut *guard;
        let now = Instant::now();
        let elapsed = now - s.counted_to;
        s.counted_to = now;
        if s.blocked.is_some() || s.playing.is_empty() {
            return;
        }
        let drops = s.playing.len() == 1 || !s.alone_only;
        for app_id in s.playing.clone() {
            let Some(f) = s.games.get_mut(&app_id) else {
                continue;
            };
            f.game.hours += elapsed.as_secs_f64() / 3600.0;
            let Some(every) = f
                .drop_every
                .filter(|_| drops && !f.game.private && f.game.hours >= f.needs_hours)
            else {
                continue;
            };
            f.since_drop += elapsed;
            let mut due = 0;
            while f.since_drop >= every && f.game.drops.remaining > due {
                f.since_drop -= every;
                due += 1;
            }
            for _ in 0..due {
                drop_card(s, app_id);
            }
        }
    }
}

/// One card drops for `app_id`: the next it was told of, as a copy of its
/// own that Steam will announce.
fn drop_card(s: &mut State, app_id: AppId) {
    let asset_id = AssetId(FIRST_ASSET + s.held.len() as u64);
    let Some(f) = s.games.get_mut(&app_id) else {
        return;
    };
    if f.game.drops.remaining == 0 {
        return;
    }
    f.game.drops.remaining -= 1;
    f.game.drops.received += 1;
    let (name, kind) = f
        .next
        .pop_front()
        .unwrap_or_else(|| (format!("Card {}", f.game.drops.received), CardKind::Normal));
    let foil = kind == CardKind::Foil;
    let set = if foil { &mut f.foils } else { &mut f.cards };
    match set.iter_mut().find(|c| c.name == name) {
        Some(card) => card.owned += 1,
        // A foil counts on its badge's page whichever it is; a normal card
        // outside the set the test gave, nowhere.
        None if foil => set.push(Card {
            name: name.clone(),
            owned: 1,
        }),
        None => {}
    }
    let market_name = if foil {
        format!("{name} (Foil)")
    } else {
        name.clone()
    };
    s.held.push(CardAsset {
        asset_id,
        app_id,
        market_hash_name: format!("{app_id}-{market_name}"),
        name,
        kind,
        marketable: true,
        tradable: true,
    });
    s.unannounced.push(NewItem {
        asset_id,
        app_id: Some(app_id),
        gained_at: None,
    });
}

#[async_trait]
impl GameRepository for FakeSteamAccount {
    /// The badge pages: every game, without its set.
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.reads += 1;
        if s.replaced {
            anyhow::bail!("the connection to Steam closed");
        }
        if s.down {
            anyhow::bail!("steamcommunity.com didn't answer (503 Service Unavailable)");
        }
        Ok(SteamLibrary::new(
            s.games.values().map(|f| f.game.clone()).collect(),
        ))
    }

    fn replaced(&self) -> bool {
        self.state.lock().unwrap().replaced
    }
}

#[async_trait]
impl CardRepository for FakeSteamAccount {
    /// A game's card page, its set and all.
    async fn game_cards(&self, app_id: AppId) -> anyhow::Result<GameCards> {
        self.settle();
        let s = self.state.lock().unwrap();
        if s.down {
            anyhow::bail!("steamcommunity.com didn't answer (503 Service Unavailable)");
        }
        s.games
            .get(&app_id)
            .map(|f| GameCards {
                game: f.game.clone(),
                set: CardSet::new(CardKind::Normal, f.cards.clone()),
            })
            .ok_or_else(|| anyhow::anyhow!("its card page has no card drops to read"))
    }

    /// A game's foil badge page: its set in foil, and how many of each.
    async fn foils(&self, app_id: AppId) -> anyhow::Result<CardSet> {
        self.settle();
        let s = self.state.lock().unwrap();
        if s.down {
            anyhow::bail!("steamcommunity.com didn't answer (503 Service Unavailable)");
        }
        s.games
            .get(&app_id)
            .map(|f| CardSet::new(CardKind::Foil, f.foils.clone()))
            .ok_or_else(|| anyhow::anyhow!("its card page has no card drops to read"))
    }

    /// The cards that dropped among these items, in the order asked.
    async fn describe(&self, asset_ids: &[AssetId]) -> anyhow::Result<Vec<CardAsset>> {
        let takes = {
            let mut s = self.state.lock().unwrap();
            s.describes.push(asset_ids.to_vec());
            s.describe_takes
        };
        tokio::time::sleep(takes).await;
        let s = self.state.lock().unwrap();
        if s.cant_describe {
            anyhow::bail!("Steam didn't answer in time");
        }
        Ok(asset_ids
            .iter()
            .filter_map(|id| s.held.iter().find(|a| a.asset_id == *id).cloned())
            .collect())
    }

    async fn next_new_items(&self) -> Vec<NewItem> {
        match self.to_announce.lock().await.recv().await {
            Some(items) => items,
            None => std::future::pending().await,
        }
    }
}

/// Signs the session on, if it isn't.
fn sign_on(s: &mut State) {
    if !s.signed_on {
        s.signed_on = true;
        s.sign_ons += 1;
        s.replaced = false;
    }
}

#[async_trait]
impl PlayingRepository for FakeSteamAccount {
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()> {
        self.settle();
        let mut s = self.state.lock().unwrap();
        sign_on(&mut s);
        if s.blocked.is_some() {
            s.playing.clear();
            return Ok(());
        }
        s.playing = app_ids.to_vec();
        s.plays.push((app_ids.to_vec(), online));
        Ok(())
    }

    async fn listen(&self) -> anyhow::Result<()> {
        sign_on(&mut self.state.lock().unwrap());
        Ok(())
    }

    async fn stop(&self) {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.playing.clear();
        s.signed_on = false;
        s.stops += 1;
    }

    fn playing(&self) -> Playing {
        let s = self.state.lock().unwrap();
        match s.blocked.filter(|_| s.signed_on) {
            Some(by) => Playing::Elsewhere(by),
            None => Playing::Here,
        }
    }

    async fn next_signal(&self) -> PlayingSignal {
        match self.to_hear.lock().await.recv().await {
            Some(signal) => signal,
            None => std::future::pending().await,
        }
    }
}
