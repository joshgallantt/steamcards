use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::{AppId, Playing, PlayingRepository, PlayingSignal};

/// Playing, held in memory: what's played, whether another device plays,
/// and what Steam says, as the test tells it.
pub struct FakePlayingRepository {
    /// Every list of games played, and whether online, in order.
    pub plays: Mutex<Vec<(Vec<AppId>, bool)>>,
    /// Another device plays: its game, when Steam says.
    pub elsewhere: Mutex<Option<Option<AppId>>>,
    pub signed_on: AtomicBool,
    pub stops: AtomicUsize,
    /// Signing on fails, as if Steam were down.
    pub down: AtomicBool,
    said: mpsc::UnboundedSender<PlayingSignal>,
    to_hear: tokio::sync::Mutex<mpsc::UnboundedReceiver<PlayingSignal>>,
}

impl Default for FakePlayingRepository {
    fn default() -> Self {
        let (said, to_hear) = mpsc::unbounded_channel();
        Self {
            plays: Mutex::default(),
            elsewhere: Mutex::default(),
            signed_on: AtomicBool::new(false),
            stops: AtomicUsize::new(0),
            down: AtomicBool::new(false),
            said,
            to_hear: tokio::sync::Mutex::new(to_hear),
        }
    }
}

impl FakePlayingRepository {
    /// Steam says this about playing, for the next to hear.
    pub fn says(&self, signal: PlayingSignal) {
        let _ = self.said.send(signal);
    }

    fn sign_on(&self) -> anyhow::Result<()> {
        if self.down.load(Ordering::Relaxed) {
            anyhow::bail!("couldn't reach Steam");
        }
        self.signed_on.store(true, Ordering::Relaxed);
        Ok(())
    }
}

#[async_trait]
impl PlayingRepository for FakePlayingRepository {
    async fn play(&self, app_ids: &[AppId], online: bool) -> anyhow::Result<()> {
        self.sign_on()?;
        if self.elsewhere.lock().unwrap().is_none() {
            self.plays.lock().unwrap().push((app_ids.to_vec(), online));
        }
        Ok(())
    }

    async fn listen(&self) -> anyhow::Result<()> {
        self.sign_on()
    }

    async fn stop(&self) {
        self.signed_on.store(false, Ordering::Relaxed);
        self.stops.fetch_add(1, Ordering::Relaxed);
    }

    fn playing(&self) -> Playing {
        match *self.elsewhere.lock().unwrap() {
            Some(by) if self.signed_on.load(Ordering::Relaxed) => Playing::Elsewhere(by),
            _ => Playing::Here,
        }
    }

    async fn next_signal(&self) -> PlayingSignal {
        match self.to_hear.lock().await.recv().await {
            Some(signal) => signal,
            None => std::future::pending().await,
        }
    }
}
