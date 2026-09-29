use std::time::{Duration, Instant};

use farming::{Game, GameListing, ListGames};
use futures::FutureExt;
use tokio::task::JoinHandle;

/// Every game with cards, read from the badges in the background, so a
/// screen showing them never waits on Steam.
pub struct Library {
    list: ListGames,
    pending: Option<JoinHandle<GameListing>>,
    listing: Option<GameListing>,
    fetched: Option<Instant>,
    /// Something the list depends on changed since the one at hand, or the
    /// one on its way, was asked for.
    outdated: bool,
    /// Why the last fetch didn't finish.
    error: Option<String>,
}

impl Library {
    pub fn new(list: ListGames) -> Self {
        Self {
            list,
            pending: None,
            listing: None,
            fetched: None,
            outdated: false,
            error: None,
        }
    }

    /// Reads the badges again. The last list stays up until the new one
    /// lands. While a read is under way, asking again does nothing, unless
    /// the list went out of date since it began.
    pub fn refresh(&mut self) {
        if self.pending.is_none() || self.outdated {
            self.pending = Some((self.list)());
            self.outdated = false;
        }
    }

    /// Reads the badges unless a whole, up-to-date list younger than
    /// `max_age` is at hand.
    pub fn refresh_if_older(&mut self, max_age: Duration) {
        let whole = self.listing.as_ref().is_some_and(|l| l.failed.is_none());
        let young = self.fetched.is_some_and(|t| t.elapsed() < max_age);
        if !(whole && young && !self.outdated) {
            self.refresh();
        }
    }

    /// The list no longer says what a read would: the account changed.
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
            Ok(listing) => {
                self.listing = Some(listing);
                self.fetched = Some(Instant::now());
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }

    pub fn is_loading(&self) -> bool {
        self.pending.is_some()
    }

    pub fn listing(&self) -> Option<&GameListing> {
        self.listing.as_ref()
    }

    /// Why the list couldn't be read, when it couldn't.
    pub fn failure(&self) -> Option<&str> {
        self.error
            .as_deref()
            .or_else(|| self.listing.as_ref()?.failed.as_deref())
    }

    /// The games with cards left, in the list's order.
    pub fn with_cards_left(&self) -> Vec<Game> {
        self.listing
            .iter()
            .flat_map(|l| &l.games)
            .filter(|g| !g.is_done())
            .cloned()
            .collect()
    }

    /// A game's name, when the list has it.
    pub fn name(&self, app_id: u32) -> Option<String> {
        self.listing
            .iter()
            .flat_map(|l| &l.games)
            .find(|g| g.app_id == app_id)
            .map(|g| g.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    fn game(app_id: u32, cards_left: u32) -> Game {
        Game {
            app_id,
            name: format!("Game {app_id}"),
            hours: 1.0,
            cards_left,
            cards_dropped: 0,
        }
    }

    /// Answers every read with `listing`, counting the reads.
    fn answering(listing: GameListing) -> (ListGames, Arc<AtomicUsize>) {
        let asked = Arc::new(AtomicUsize::new(0));
        let count = asked.clone();
        let list: ListGames = Arc::new(move || {
            count.fetch_add(1, Ordering::Relaxed);
            let listing = listing.clone();
            tokio::spawn(async move { listing })
        });
        (list, asked)
    }

    async fn settle(l: &mut Library) {
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
    async fn reads_in_the_background_and_lists_games_with_cards_left() {
        let (list, asked) = answering(GameListing {
            games: vec![game(620, 3), game(220, 0)],
            failed: None,
        });
        let mut l = Library::new(list);
        l.refresh();
        l.refresh();
        assert_eq!(asked.load(Ordering::Relaxed), 1, "one read at a time");
        settle(&mut l).await;
        let left: Vec<u32> = l.with_cards_left().iter().map(|g| g.app_id).collect();
        assert_eq!(left, [620]);
        assert_eq!(l.name(220).as_deref(), Some("Game 220"));
    }

    #[tokio::test]
    async fn a_fresh_list_is_not_read_again_but_a_failed_one_is() {
        let (list, asked) = answering(GameListing::default());
        let mut l = Library::new(list);
        l.refresh_if_older(Duration::from_secs(600));
        settle(&mut l).await;
        l.refresh_if_older(Duration::from_secs(600));
        assert_eq!(asked.load(Ordering::Relaxed), 1);

        let (list, asked) = answering(GameListing {
            games: Vec::new(),
            failed: Some("steamcommunity.com didn't answer".into()),
        });
        let mut l = Library::new(list);
        l.refresh_if_older(Duration::from_secs(600));
        settle(&mut l).await;
        assert_eq!(l.failure(), Some("steamcommunity.com didn't answer"));
        l.refresh_if_older(Duration::from_secs(600));
        assert_eq!(asked.load(Ordering::Relaxed), 2, "tried again");
        settle(&mut l).await;
    }
}
