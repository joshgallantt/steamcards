use std::sync::Arc;

use preferences::{GetPreferencesUseCase, PreferencesError, SetAutoUpdateUseCase};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use update::{KeepUpToDateUseCase, UpdateEvent};

/// Keeping steamcards up to date: runs the use case that does it, passes on
/// what it finds, and turns it on or off.
pub struct UpdateViewModel {
    keep_up_to_date: Arc<dyn KeepUpToDateUseCase>,
    get: Arc<dyn GetPreferencesUseCase>,
    set_auto_update: Arc<dyn SetAutoUpdateUseCase>,
    tx: mpsc::Sender<UpdateEvent>,
    events: mpsc::Receiver<UpdateEvent>,
    keeping: Option<CancellationToken>,
    /// The last thing it found, for the account pop-up.
    last: Option<UpdateEvent>,
}

impl UpdateViewModel {
    pub fn new(
        keep_up_to_date: Arc<dyn KeepUpToDateUseCase>,
        get: Arc<dyn GetPreferencesUseCase>,
        set_auto_update: Arc<dyn SetAutoUpdateUseCase>,
    ) -> Self {
        let (tx, events) = mpsc::channel(8);
        Self {
            keep_up_to_date,
            get,
            set_auto_update,
            tx,
            events,
            keeping: None,
            last: None,
        }
    }

    /// Starts keeping up to date in the background, if it isn't already.
    pub fn start(&mut self) {
        if self.keeping.is_none() {
            let token = CancellationToken::new();
            drop(self.keep_up_to_date.call(token.clone(), self.tx.clone()));
            self.keeping = Some(token);
        }
    }

    pub fn stop(&mut self) {
        if let Some(token) = self.keeping.take() {
            token.cancel();
        }
    }

    /// What it found since last asked.
    pub fn try_recv(&mut self) -> Option<UpdateEvent> {
        let found = self.events.try_recv().ok()?;
        self.last = Some(found);
        Some(found)
    }

    /// The last thing it found, if anything.
    pub fn last(&self) -> Option<UpdateEvent> {
        self.last
    }

    pub fn auto_update(&self) -> bool {
        self.get.call().auto_update
    }

    /// Turns keeping up to date on or off. Turned on, it looks again at
    /// once, rather than a day later.
    pub fn toggle_auto_update(&mut self) -> Result<(), PreferencesError> {
        let on = !self.auto_update();
        self.set_auto_update.call(on)?;
        if on {
            self.stop();
            self.start();
        }
        Ok(())
    }
}

impl Drop for UpdateViewModel {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use preferences::{
        DefaultGetPreferencesUseCase, DefaultSetAutoUpdateUseCase,
        test_support::FakePreferencesRepository,
    };
    use update::{UpdatedBy, Version, test_support::SpyKeepUpToDateUseCase};

    use super::*;

    const FOUND: UpdateEvent = UpdateEvent::Available {
        version: Version::new(0, 1, 3),
        by: UpdatedBy::Homebrew,
    };

    fn updates(spy: Arc<SpyKeepUpToDateUseCase>) -> UpdateViewModel {
        let repo = Arc::new(FakePreferencesRepository::default());
        UpdateViewModel::new(
            spy,
            Arc::new(DefaultGetPreferencesUseCase::new(repo.clone())),
            Arc::new(DefaultSetAutoUpdateUseCase::new(repo)),
        )
    }

    #[tokio::test]
    async fn what_it_finds_is_passed_on_and_kept() {
        let spy = Arc::new(SpyKeepUpToDateUseCase {
            says: vec![FOUND],
            ..Default::default()
        });
        let mut u = updates(spy.clone());

        u.start();
        u.start();
        tokio::task::yield_now().await;

        assert_eq!(spy.starts.load(Ordering::Relaxed), 1, "started once");
        assert_eq!(u.try_recv(), Some(FOUND));
        assert_eq!(u.last(), Some(FOUND));
    }

    #[tokio::test]
    async fn turned_on_again_it_looks_at_once() {
        let spy = Arc::new(SpyKeepUpToDateUseCase::default());
        let mut u = updates(spy.clone());
        u.start();
        assert!(u.auto_update(), "on at first");

        u.toggle_auto_update().unwrap();
        assert!(!u.auto_update());
        u.toggle_auto_update().unwrap();

        assert!(u.auto_update());
        assert_eq!(spy.starts.load(Ordering::Relaxed), 2);
    }
}
