use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use card::AssetId;
use chrono::DateTime;
use farming::Signal;
use session::NewItem;
use steam_api::{
    EResult, SteamClient,
    cm::{Announcement, Connection, Event, UnseenItem},
};
use steam_library::AppId;
use tokio::sync::broadcast;

/// Steam's side of farming: what's played, and what Steam says back.
#[async_trait]
pub trait FarmingClient: Send + Sync {
    /// Tells Steam these games are played, and whether to show online.
    /// While another device plays, it's told nothing: see `blocked`.
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()>;

    /// Signs on to hear what Steam says, playing nothing.
    async fn listen(&self) -> anyhow::Result<()>;

    /// Stops playing, and signs off.
    async fn stop(&self);

    /// Whether another device is playing, and its game when Steam says:
    /// `None` when none is.
    fn blocked(&self) -> Option<Option<AppId>>;

    /// What Steam says next.
    async fn next_signal(&self) -> Signal;
}

/// Playing games on the Steam session's CM connection, as the Steam client
/// does: nothing is launched, Steam is told what's being played. While
/// another device plays, nothing is: Steam signs off a session that says
/// it's playing then.
pub struct SteamFarmingClient {
    steam: Arc<SteamClient>,
    /// The connection being played on, and what it was last told: a
    /// connection that went is replaced, and the new one told afresh.
    on: Mutex<Option<Played>>,
    /// What that connection says.
    news: tokio::sync::Mutex<Option<broadcast::Receiver<Event>>>,
    /// The new items heard of, on any connection.
    heard: Mutex<Heard>,
}

struct Played {
    conn: Arc<Connection>,
    /// The account it signed on as.
    account: Option<u64>,
    games: Vec<u32>,
    online: bool,
}

/// What Steam has said is new. It says so again with every announcement
/// until the inventory is viewed, so each item is passed on once.
#[derive(Default)]
struct Heard {
    items: HashSet<u64>,
    /// How many new items Steam last counted.
    count: u32,
    /// The account whose first sign-on here was taken as what was new
    /// already. Signing on again, it's what's new since.
    baseline: Option<u64>,
    /// What a sign-on said was new, heard as a connection was taken up,
    /// before anything asked: passed on with the next signal.
    waiting: Option<Vec<NewItem>>,
}

impl Heard {
    /// Takes in an announcement on a connection signed on as `account`: the
    /// community items it lists that weren't heard of before. When it lists
    /// none such but counts more than before, none: a card may have dropped
    /// all the same. `None` when nothing is new, or when it's what was there
    /// already when the account first signed on here.
    fn hear(&mut self, a: &Announcement, account: Option<u64>) -> Option<Vec<NewItem>> {
        let fresh: Vec<NewItem> = a
            .items
            .iter()
            .filter(|i| i.is_community_item() && self.items.insert(i.asset_id))
            .map(new_item)
            .collect();
        let more = a.count > self.count;
        self.count = a.count;
        if a.at_sign_on && (account.is_none() || self.baseline != account) {
            self.baseline = account;
            return None;
        }
        (!fresh.is_empty() || more).then_some(fresh)
    }

    /// Keeps what a sign-on said was new for the next signal.
    fn keep(&mut self, items: Vec<NewItem>) {
        self.waiting.get_or_insert_default().extend(items);
    }
}

impl SteamFarmingClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self {
            steam,
            on: Mutex::default(),
            news: tokio::sync::Mutex::default(),
            heard: Mutex::default(),
        }
    }

    /// The signed-on connection, signing on first if need be, and what it
    /// was last told: the games, and whether it shows online. A new one is
    /// heard from here on, what was new when it signed on included, once
    /// Steam has said whether another device is playing. It's told nothing
    /// before.
    async fn take_up(&self) -> anyhow::Result<(Arc<Connection>, Vec<u32>, bool)> {
        let conn = self.steam.connection().await?;
        let told = {
            let on = self.on.lock().unwrap();
            on.as_ref()
                .filter(|p| Arc::ptr_eq(&p.conn, &conn))
                .map(|p| (p.games.clone(), p.online))
        };
        if let Some((games, online)) = told {
            return Ok((conn, games, online));
        }
        let news = conn.events();
        conn.playing_state().await;
        *self.news.lock().await = Some(news);
        if let Some(already) = conn.new_at_sign_on() {
            let mut heard = self.heard.lock().unwrap();
            if let Some(items) = heard.hear(&already, conn.steam_id()) {
                heard.keep(items);
            }
        }
        // A session plays nothing, and is offline, until it says otherwise.
        *self.on.lock().unwrap() = Some(Played {
            account: conn.steam_id(),
            conn: conn.clone(),
            games: Vec::new(),
            online: false,
        });
        Ok((conn, Vec::new(), false))
    }

    /// Lets the connection played on go. If the account's first sign-on
    /// went unheard, what it said was new is heard first, as what was there
    /// already: signing on again then says what's new since.
    fn let_go(&self) -> Option<Played> {
        let played = self.on.lock().unwrap().take();
        if let Some(p) = &played
            && let Some(already) = p.conn.new_at_sign_on()
        {
            let mut heard = self.heard.lock().unwrap();
            if heard.baseline != p.account {
                heard.hear(&already, p.account);
            }
        }
        played
    }
}

#[async_trait]
impl FarmingClient for SteamFarmingClient {
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()> {
        let (conn, told, was_online) = self.take_up().await?;
        if online != was_online {
            conn.set_online(online)?;
        }
        // While another device plays, Steam signs off a session that says
        // it's playing, and counts nothing of this one's: nothing is said.
        let games = if conn.blocked().is_some_and(|b| b.blocked) {
            Vec::new()
        } else {
            let games: Vec<u32> = app_ids.iter().map(|id| id.0).collect();
            if games != told {
                conn.play(&games)?;
            }
            games
        };
        *self.on.lock().unwrap() = Some(Played {
            account: conn.steam_id(),
            conn,
            games,
            online,
        });
        Ok(())
    }

    async fn listen(&self) -> anyhow::Result<()> {
        self.take_up().await.map(|_| ())
    }

    async fn stop(&self) {
        let played = self.let_go();
        if let Some(p) = played.filter(|p| !p.games.is_empty()) {
            let _ = p.conn.play(&[]);
        }
        *self.news.lock().await = None;
        self.steam.disconnect().await;
    }

    fn blocked(&self) -> Option<Option<AppId>> {
        self.steam
            .current()?
            .blocked()
            .filter(|b| b.blocked)
            .map(|b| b.app_id.map(AppId))
    }

    async fn next_signal(&self) -> Signal {
        if let Some(items) = self.heard.lock().unwrap().waiting.take() {
            return Signal::NewItems(items);
        }
        let mut news = self.news.lock().await;
        loop {
            let Some(rx) = news.as_mut() else {
                // Nothing connected, so nothing to hear until something is.
                drop(news);
                return std::future::pending().await;
            };
            match rx.recv().await {
                Ok(Event::PlayingBlocked(b)) if b.blocked => {
                    // Nothing this session plays counts meanwhile: once it
                    // stops, the games are told again.
                    if let Some(p) = self.on.lock().unwrap().as_mut() {
                        p.games.clear();
                    }
                    return Signal::Blocked(b.app_id.map(AppId));
                }
                Ok(Event::PlayingBlocked(_)) => return Signal::Unblocked,
                Ok(Event::NewItems(announced)) => {
                    let account = self.on.lock().unwrap().as_ref().and_then(|p| p.account);
                    if let Some(items) = self.heard.lock().unwrap().hear(&announced, account) {
                        return Signal::NewItems(items);
                    }
                }
                Ok(Event::LoggedOff(why)) => {
                    *news = None;
                    self.let_go();
                    return match why {
                        EResult::LOGON_SESSION_REPLACED => Signal::Replaced,
                        EResult::LOGGED_IN_ELSEWHERE => Signal::TakenOver,
                        why => Signal::Lost(format!("Steam signed this session off ({why})")),
                    };
                }
                Ok(Event::Closed) | Err(broadcast::error::RecvError::Closed) => {
                    *news = None;
                    self.let_go();
                    return Signal::Lost("the connection to Steam dropped".into());
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    }
}

/// An item Steam listed, as farming knows it: the game it came from is its
/// source app.
fn new_item(item: &UnseenItem) -> NewItem {
    NewItem {
        asset_id: AssetId(item.asset_id),
        app_id: item.source_app_id.map(AppId),
        gained_at: item
            .gained_at
            .and_then(|at| DateTime::from_timestamp(i64::from(at), 0)),
    }
}
