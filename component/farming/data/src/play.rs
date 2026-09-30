use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use chrono::DateTime;
use farming::{NewItem, PlayRepository, Signal};
use keep_awake::KeepAwake;
use steam_api::{
    EResult, Session,
    cm::{Announcement, Connection, Event, UnseenItem},
};
use tokio::sync::broadcast;

/// Playing games on the Steam session's CM connection, as the Steam client
/// does: nothing is launched, Steam is told what's being played. While
/// anything is, the computer is kept awake.
pub struct SteamPlayRepository {
    session: Arc<Session>,
    awake: Arc<KeepAwake>,
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
}

impl Heard {
    /// Takes in an announcement: the community items it lists that weren't
    /// heard of before. When it lists none such but counts more than
    /// before, none: a card may have dropped all the same. `None` when
    /// nothing is new, or when it's what was there already at sign-on.
    fn hear(&mut self, a: &Announcement) -> Option<Vec<NewItem>> {
        let fresh: Vec<NewItem> = a
            .items
            .iter()
            .filter(|i| i.is_community_item() && self.items.insert(i.asset_id))
            .map(new_item)
            .collect();
        let more = a.count > self.count;
        self.count = a.count;
        if a.at_sign_on {
            return None;
        }
        (!fresh.is_empty() || more).then_some(fresh)
    }
}

impl SteamPlayRepository {
    pub fn new(session: Arc<Session>, awake: Arc<KeepAwake>) -> Self {
        Self {
            session,
            awake,
            on: Mutex::default(),
            news: tokio::sync::Mutex::default(),
            heard: Mutex::default(),
        }
    }
}

#[async_trait]
impl PlayRepository for SteamPlayRepository {
    async fn play(&self, app_ids: &[u32], online: bool) -> anyhow::Result<()> {
        let conn = self.session.connection().await?;
        let before = {
            let on = self.on.lock().unwrap();
            on.as_ref()
                .filter(|p| Arc::ptr_eq(&p.conn, &conn))
                .map(|p| (p.games.clone(), p.online))
        };
        if before.is_none() {
            // A new connection: hear what it says from here on, what was
            // new when it signed on included.
            *self.news.lock().await = Some(conn.events());
            if let Some(already) = conn.new_at_sign_on() {
                self.heard.lock().unwrap().hear(&already);
            }
        }
        // A session is offline until it says otherwise.
        let was_online = before.as_ref().is_some_and(|(_, o)| *o);
        if online != was_online {
            conn.set_online(online)?;
        }
        if before.as_ref().is_none_or(|(games, _)| games != app_ids) {
            conn.play(app_ids)?;
        }
        *self.on.lock().unwrap() = Some(Played {
            conn,
            games: app_ids.to_vec(),
            online,
        });
        if app_ids.is_empty() {
            self.awake.let_sleep();
        } else {
            self.awake.hold();
        }
        Ok(())
    }

    async fn stop(&self) {
        let played = self.on.lock().unwrap().take();
        if let Some(p) = played.filter(|p| !p.games.is_empty()) {
            let _ = p.conn.play(&[]);
        }
        *self.news.lock().await = None;
        self.awake.let_sleep();
        self.session.disconnect().await;
    }

    fn blocked(&self) -> Option<Option<u32>> {
        self.session
            .current()?
            .blocked()
            .filter(|b| b.blocked)
            .map(|b| b.app_id)
    }

    async fn next_signal(&self) -> Signal {
        let mut news = self.news.lock().await;
        loop {
            let Some(rx) = news.as_mut() else {
                // Nothing connected, so nothing to hear until something is.
                drop(news);
                return std::future::pending().await;
            };
            match rx.recv().await {
                Ok(Event::PlayingBlocked(b)) if b.blocked => return Signal::Blocked(b.app_id),
                Ok(Event::PlayingBlocked(_)) => return Signal::Unblocked,
                Ok(Event::NewItems(announced)) => {
                    if let Some(items) = self.heard.lock().unwrap().hear(&announced) {
                        return Signal::NewItems(items);
                    }
                }
                Ok(Event::LoggedOff(EResult::LOGON_SESSION_REPLACED)) => {
                    *news = None;
                    self.on.lock().unwrap().take();
                    return Signal::Replaced;
                }
                Ok(Event::LoggedOff(why)) => {
                    *news = None;
                    self.on.lock().unwrap().take();
                    return Signal::Lost(signed_off(why));
                }
                Ok(Event::Closed) | Err(broadcast::error::RecvError::Closed) => {
                    *news = None;
                    self.on.lock().unwrap().take();
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
        asset_id: item.asset_id,
        app_id: item.source_app_id,
        gained_at: item
            .gained_at
            .and_then(|at| DateTime::from_timestamp(i64::from(at), 0)),
    }
}

/// Why Steam signed the session off, in the user's terms.
fn signed_off(why: EResult) -> String {
    match why {
        EResult::LOGGED_IN_ELSEWHERE => {
            "Steam signed this session off: the account signed in elsewhere".into()
        }
        why => format!("Steam signed this session off ({why})"),
    }
}
