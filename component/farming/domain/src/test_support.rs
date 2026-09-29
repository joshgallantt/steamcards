//! Doubles for this crate's tests and other crates' tests. A double stands in
//! for the contract, so no test needs Steam.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use library::{CardDrops, Game, LibraryRepository, SteamLibrary};
use tokio::{sync::mpsc, time::Instant};

use crate::{FarmCards, PlayRepository, Signal};

/// Never farms. For screens that need a farmer to exist.
pub fn idle_farmer() -> FarmCards {
    Arc::new(|_, _| tokio::spawn(async {}))
}

/// A game's card drops, as the stand-in plays them out.
#[derive(Debug, Clone)]
struct Farmed {
    game: Game,
    /// Played time from one card to the next; `None` never drops.
    drop_every: Option<Duration>,
    /// Hours it needs before its cards drop at all.
    needs_hours: f64,
    /// Played time since the last card dropped.
    since_drop: Duration,
}

struct State {
    games: BTreeMap<u32, Farmed>,
    /// Cards drop only for a game played alone.
    alone_only: bool,
    playing: Vec<u32>,
    /// Up to when what's playing has been counted.
    counted_to: Instant,
    blocked: Option<Option<u32>>,
    plays: Vec<(Vec<u32>, bool)>,
    stops: usize,
    reads: usize,
    down: bool,
}

/// Steam, as far as farming sees it, in memory: a library whose badges answer
/// as its games have been played, on tokio's clock, so a test can run hours
/// of farming on paused time.
pub struct InMemorySteam {
    state: Mutex<State>,
    signals_tx: mpsc::UnboundedSender<Signal>,
    signals: tokio::sync::Mutex<mpsc::UnboundedReceiver<Signal>>,
}

impl Default for InMemorySteam {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySteam {
    pub fn new() -> Self {
        let (signals_tx, signals) = mpsc::unbounded_channel();
        Self {
            state: Mutex::new(State {
                games: BTreeMap::new(),
                alone_only: false,
                playing: Vec::new(),
                counted_to: Instant::now(),
                blocked: None,
                plays: Vec::new(),
                stops: 0,
                reads: 0,
                down: false,
            }),
            signals_tx,
            signals: tokio::sync::Mutex::new(signals),
        }
    }

    /// Adds "Game <app ID>", with `hours` played and `cards` drops to come:
    /// one every `every` of play once it has 3 hours (`None`: never).
    pub fn add_game(&self, app_id: u32, hours: f64, cards: u32, every: Option<Duration>) {
        self.state.lock().unwrap().games.insert(
            app_id,
            Farmed {
                game: Game {
                    app_id,
                    name: format!("Game {app_id}"),
                    hours,
                    drops: CardDrops {
                        received: 0,
                        remaining: cards,
                    },
                    badge_level: 0,
                    cards: Vec::new(),
                },
                drop_every: every,
                needs_hours: 3.0,
                since_drop: Duration::ZERO,
            },
        );
    }

    /// Cards drop only for a game played on its own.
    pub fn drops_only_alone(&self) {
        self.state.lock().unwrap().alone_only = true;
    }

    /// Another device starts playing `app_id` on the account.
    pub fn block(&self, app_id: Option<u32>) {
        self.settle();
        self.state.lock().unwrap().blocked = Some(app_id);
        let _ = self.signals_tx.send(Signal::Blocked(app_id));
    }

    /// It stops.
    pub fn unblock(&self) {
        self.settle();
        self.state.lock().unwrap().blocked = None;
        let _ = self.signals_tx.send(Signal::Unblocked);
    }

    /// Steam says new items arrived.
    pub fn new_items(&self) {
        let _ = self.signals_tx.send(Signal::NewItems);
    }

    /// Another session signs on in this one's place.
    pub fn replace(&self) {
        let _ = self.signals_tx.send(Signal::Replaced);
    }

    /// The connection to Steam goes.
    pub fn lose(&self, why: &str) {
        let _ = self.signals_tx.send(Signal::Lost(why.into()));
    }

    /// The badges can't be read, as if steamcommunity.com were down, or can
    /// again.
    pub fn go_down(&self, down: bool) {
        self.state.lock().unwrap().down = down;
    }

    /// Every list of games played, and whether online, in order.
    pub fn plays(&self) -> Vec<(Vec<u32>, bool)> {
        self.state.lock().unwrap().plays.clone()
    }

    /// Just the games of each play.
    pub fn played(&self) -> Vec<Vec<u32>> {
        self.plays().into_iter().map(|(games, _)| games).collect()
    }

    /// What's playing now.
    pub fn playing(&self) -> Vec<u32> {
        self.state.lock().unwrap().playing.clone()
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
            .get(&app_id)
            .map(|f| f.game.clone())
    }

    /// Counts the time since last counted for what's playing: hours for each
    /// game, and the cards that drop.
    fn settle(&self) {
        let mut s = self.state.lock().unwrap();
        let now = Instant::now();
        let elapsed = now - s.counted_to;
        s.counted_to = now;
        if s.blocked.is_some() || s.playing.is_empty() {
            return;
        }
        let drops = s.playing.len() == 1 || !s.alone_only;
        let playing = s.playing.clone();
        for app_id in playing {
            let Some(f) = s.games.get_mut(&app_id) else {
                continue;
            };
            f.game.hours += elapsed.as_secs_f64() / 3600.0;
            let Some(every) = f
                .drop_every
                .filter(|_| drops && f.game.hours >= f.needs_hours)
            else {
                continue;
            };
            f.since_drop += elapsed;
            while f.since_drop >= every && f.game.drops.remaining > 0 {
                f.since_drop -= every;
                f.game.drops.remaining -= 1;
                f.game.drops.received += 1;
            }
        }
    }
}

#[async_trait]
impl LibraryRepository for InMemorySteam {
    async fn library(&self) -> anyhow::Result<SteamLibrary> {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.reads += 1;
        if s.down {
            anyhow::bail!("steamcommunity.com didn't answer (503 Service Unavailable)");
        }
        Ok(SteamLibrary::new(
            s.games.values().map(|f| f.game.clone()).collect(),
        ))
    }

    async fn game(&self, app_id: u32) -> anyhow::Result<Game> {
        self.settle();
        let s = self.state.lock().unwrap();
        if s.down {
            anyhow::bail!("steamcommunity.com didn't answer (503 Service Unavailable)");
        }
        s.games
            .get(&app_id)
            .map(|f| f.game.clone())
            .ok_or_else(|| anyhow::anyhow!("its card page has no card drops to read"))
    }
}

#[async_trait]
impl PlayRepository for InMemorySteam {
    async fn play(&self, app_ids: &[u32], online: bool) -> anyhow::Result<()> {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.playing = app_ids.to_vec();
        s.plays.push((app_ids.to_vec(), online));
        Ok(())
    }

    async fn stop(&self) {
        self.settle();
        let mut s = self.state.lock().unwrap();
        s.playing.clear();
        s.stops += 1;
    }

    fn blocked(&self) -> Option<Option<u32>> {
        self.state.lock().unwrap().blocked
    }

    async fn next_signal(&self) -> Signal {
        match self.signals.lock().await.recv().await {
            Some(signal) => signal,
            None => std::future::pending().await,
        }
    }
}
