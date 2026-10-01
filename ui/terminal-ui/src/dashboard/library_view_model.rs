use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use futures::FutureExt;
use game::{Game, GameError, GetLibraryUseCase, SteamLibrary};
use tokio::task::JoinHandle;

/// The Steam library, read in the background, so a screen showing it never
/// waits on Steam.
pub struct LibraryViewModel {
    read: Arc<dyn GetLibraryUseCase>,
    pending: Option<JoinHandle<Result<SteamLibrary, GameError>>>,
    library: Option<SteamLibrary>,
    fetched: Option<Instant>,
    /// Something the library depends on changed since the one at hand, or
    /// the one on its way, was asked for.
    outdated: bool,
    /// Why the last read didn't work.
    error: Option<String>,
}

impl LibraryViewModel {
    pub fn new(read: Arc<dyn GetLibraryUseCase>) -> Self {
        Self {
            read,
            pending: None,
            library: None,
            fetched: None,
            outdated: false,
            error: None,
        }
    }

    /// Reads the library again. The last one stays up until the new one
    /// lands. While a read is under way, asking again does nothing, unless
    /// the library went out of date since it began.
    pub fn refresh(&mut self) {
        if self.pending.is_none() || self.outdated {
            self.pending = Some(self.read.call());
            self.outdated = false;
        }
    }

    /// Reads the library unless an up-to-date one younger than `max_age` is
    /// at hand.
    pub fn refresh_if_older(&mut self, max_age: Duration) {
        let young = self.fetched.is_some_and(|t| t.elapsed() < max_age);
        if !(young && !self.outdated && self.error.is_none()) {
            self.refresh();
        }
    }

    /// The library no longer says what a read would: the account changed.
    pub fn invalidate(&mut self) {
        self.outdated = true;
    }

    /// Takes in a finished read, if there is one. Never waits.
    pub fn poll(&mut self) {
        let Some(read) = self.pending.as_mut().filter(|f| f.is_finished()) else {
            return;
        };
        let Some(done) = read.now_or_never() else {
            return;
        };
        self.pending = None;
        match done {
            Ok(Ok(library)) => {
                self.library = Some(library);
                self.fetched = Some(Instant::now());
                self.error = None;
            }
            Ok(Err(e)) => self.error = Some(e.to_string()),
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    pub fn library(&self) -> Option<&SteamLibrary> {
        self.library.as_ref()
    }

    /// Why the last read didn't work, when it didn't.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The games with drops left, in the library's order.
    pub fn with_drops_left(&self) -> Vec<Game> {
        self.library
            .iter()
            .flat_map(|l| l.with_drops_left())
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use game::test_support::{SpyGetLibraryUseCase, game};

    use super::*;

    fn answering(result: Result<SteamLibrary, GameError>) -> Arc<SpyGetLibraryUseCase> {
        Arc::new(SpyGetLibraryUseCase::answering(result))
    }

    async fn settle(l: &mut LibraryViewModel) {
        for _ in 0..100 {
            l.poll();
            if !l.is_loading() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("the read never landed");
    }

    #[tokio::test]
    async fn reads_in_the_background_one_read_at_a_time() {
        let read = answering(Ok(SteamLibrary::new(vec![
            game(620, 5.2, 1, 3),
            game(220, 30.0, 3, 0),
        ])));
        let mut l = LibraryViewModel::new(read.clone());
        l.refresh();
        l.refresh();
        assert_eq!(read.reads(), 1);
        settle(&mut l).await;
        let left: Vec<u32> = l.with_drops_left().iter().map(|g| g.app_id.0).collect();
        assert_eq!(left, [620]);
    }

    #[tokio::test]
    async fn a_fresh_library_isnt_read_again_but_a_failed_one_is() {
        let read = answering(Ok(SteamLibrary::default()));
        let mut l = LibraryViewModel::new(read.clone());
        l.refresh_if_older(Duration::from_secs(600));
        settle(&mut l).await;
        l.refresh_if_older(Duration::from_secs(600));
        assert_eq!(read.reads(), 1);

        let read = answering(Err(GameError::Unavailable(
            "steamcommunity.com didn't answer".into(),
        )));
        let mut l = LibraryViewModel::new(read.clone());
        l.refresh_if_older(Duration::from_secs(600));
        settle(&mut l).await;
        assert_eq!(l.error(), Some("steamcommunity.com didn't answer"));
        l.refresh_if_older(Duration::from_secs(600));
        assert_eq!(read.reads(), 2, "tried again");
        settle(&mut l).await;
    }
}
