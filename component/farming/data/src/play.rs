use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use farming::{PlayRepository, Signal};
use steam_api::{
    EResult, Session,
    cm::{Connection, Event},
};
use tokio::sync::broadcast;

/// Playing games on the Steam session's CM connection, as the Steam client
/// does: nothing is launched, Steam is told what's being played.
pub struct SteamPlayRepository {
    session: Arc<Session>,
    /// The connection being played on, and what it was last told: a
    /// connection that went is replaced, and the new one told afresh.
    on: Mutex<Option<Played>>,
    /// What that connection says.
    news: tokio::sync::Mutex<Option<broadcast::Receiver<Event>>>,
}

struct Played {
    conn: Arc<Connection>,
    games: Vec<u32>,
    online: bool,
}

impl SteamPlayRepository {
    pub fn new(session: Arc<Session>) -> Self {
        Self {
            session,
            on: Mutex::default(),
            news: tokio::sync::Mutex::default(),
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
            // A new connection: hear what it says from here on.
            *self.news.lock().await = Some(conn.events());
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
        Ok(())
    }

    async fn stop(&self) {
        let played = self.on.lock().unwrap().take();
        if let Some(p) = played.filter(|p| !p.games.is_empty()) {
            let _ = p.conn.play(&[]);
        }
        *self.news.lock().await = None;
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
                Ok(Event::NewItems(n)) if n > 0 => return Signal::NewItems,
                Ok(Event::NewItems(_)) => {}
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

/// Why Steam signed the session off, in the user's terms.
fn signed_off(why: EResult) -> String {
    match why {
        EResult::LOGGED_IN_ELSEWHERE => {
            "Steam signed this session off: the account signed in elsewhere".into()
        }
        EResult::LOGON_SESSION_REPLACED => {
            "Steam signed this session off: another steamcards took its place".into()
        }
        why => format!("Steam signed this session off ({why})"),
    }
}
