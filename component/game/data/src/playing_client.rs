use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use game::{AppId, PlayingSignal};
use steam_api::{
    EResult, SteamClient,
    cm::{Connection, Event},
};
use tokio::sync::broadcast;

/// Steam's side of playing: what's played, and what Steam says back.
#[async_trait]
pub trait PlayingClient: Send + Sync {
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

    /// What Steam says next about playing.
    async fn next_signal(&self) -> PlayingSignal;
}

/// Playing games on the Steam session's CM connection, as the Steam client
/// does: nothing is launched, Steam is told what's being played. While
/// another device plays, nothing is: Steam signs off a session that says
/// it's playing then.
pub struct SteamPlayingClient {
    steam: Arc<SteamClient>,
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

impl SteamPlayingClient {
    pub fn new(steam: Arc<SteamClient>) -> Self {
        Self {
            steam,
            on: Mutex::default(),
            news: tokio::sync::Mutex::default(),
        }
    }

    /// The signed-on connection, signing on first if need be, and what it
    /// was last told: the games, and whether it shows online. A new one is
    /// heard from here on, once Steam has said whether another device is
    /// playing. It's told nothing before.
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
        // A session plays nothing, and is offline, until it says otherwise.
        *self.on.lock().unwrap() = Some(Played {
            conn: conn.clone(),
            games: Vec::new(),
            online: false,
        });
        Ok((conn, Vec::new(), false))
    }
}

#[async_trait]
impl PlayingClient for SteamPlayingClient {
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
        let played = self.on.lock().unwrap().take();
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

    async fn next_signal(&self) -> PlayingSignal {
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
                    return PlayingSignal::Blocked(b.app_id.map(AppId));
                }
                Ok(Event::PlayingBlocked(_)) => return PlayingSignal::Unblocked,
                Ok(Event::LoggedOff(why)) => {
                    *news = None;
                    self.on.lock().unwrap().take();
                    return match why {
                        EResult::LOGON_SESSION_REPLACED => PlayingSignal::Replaced,
                        EResult::LOGGED_IN_ELSEWHERE => PlayingSignal::TakenOver,
                        why => {
                            PlayingSignal::Lost(format!("Steam signed this session off ({why})"))
                        }
                    };
                }
                Ok(Event::Closed) | Err(broadcast::error::RecvError::Closed) => {
                    *news = None;
                    self.on.lock().unwrap().take();
                    return PlayingSignal::Lost("the connection to Steam dropped".into());
                }
                // New items are the card component's to hear.
                Ok(Event::NewItems(_)) | Err(broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    }
}
