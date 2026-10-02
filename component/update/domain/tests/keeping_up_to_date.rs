//! Paused-time tier: keeping steamcards up to date, over days that pass in
//! moments: the use case over a fake repository of releases. Where releases
//! come from, and how one is put in place, is tested against a stand-in for
//! GitHub in update-data.

use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use preferences::{Preferences, test_support::StubGetPreferencesUseCase};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;
use update::{
    DefaultKeepUpToDateUseCase, KeepUpToDateUseCase, UpdateEvent, UpdatedBy, Version,
    test_support::FakeReleaseRepository,
};

const HOUR: Duration = Duration::from_secs(60 * 60);
const DAY: Duration = Duration::from_secs(24 * 60 * 60);
const RUNNING: Version = Version::new(0, 1, 2);
const NEXT: Version = Version::new(0, 1, 3);

/// A copy of steamcards, `RUNNING`, keeping itself up to date.
struct Running {
    releases: Arc<FakeReleaseRepository>,
    events: mpsc::Receiver<UpdateEvent>,
    token: CancellationToken,
    _keeping: JoinHandle<()>,
}

impl Running {
    fn start(by: UpdatedBy, releases: FakeReleaseRepository, prefs: Preferences) -> Self {
        let releases = Arc::new(releases);
        let get = Arc::new(StubGetPreferencesUseCase::default());
        get.set(prefs);
        let (tx, events) = mpsc::channel(8);
        let token = CancellationToken::new();
        let keeping = DefaultKeepUpToDateUseCase::new(releases.clone(), get, RUNNING, by)
            .call(token.clone(), tx);
        Self {
            releases,
            events,
            token,
            _keeping: keeping,
        }
    }

    /// What it says within `d`, if anything.
    async fn says_within(&mut self, d: Duration) -> Option<UpdateEvent> {
        tokio::time::timeout(d, self.events.recv())
            .await
            .ok()
            .flatten()
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        self.token.cancel();
    }
}

#[tokio::test(start_paused = true)]
async fn a_newer_release_is_put_in_place_for_the_next_start() {
    let mut copy = Running::start(
        UpdatedBy::Itself,
        FakeReleaseRepository::with(NEXT),
        Preferences::default(),
    );

    assert_eq!(
        copy.says_within(HOUR).await,
        Some(UpdateEvent::Installed(NEXT))
    );
    assert_eq!(*copy.releases.installed.lock().unwrap(), [NEXT]);
}

#[tokio::test(start_paused = true)]
async fn a_copy_homebrew_looks_after_is_told_of_the_release_and_left_be() {
    let mut copy = Running::start(
        UpdatedBy::Homebrew,
        FakeReleaseRepository::with(NEXT),
        Preferences::default(),
    );

    assert_eq!(
        copy.says_within(HOUR).await,
        Some(UpdateEvent::Available {
            version: NEXT,
            by: UpdatedBy::Homebrew
        })
    );
    assert!(copy.releases.installed.lock().unwrap().is_empty());
}

#[tokio::test(start_paused = true)]
async fn the_release_running_already_is_no_news() {
    let mut copy = Running::start(
        UpdatedBy::Itself,
        FakeReleaseRepository::with(RUNNING),
        Preferences::default(),
    );

    assert_eq!(copy.says_within(2 * DAY).await, None);
    assert_eq!(copy.releases.asks.load(Ordering::Relaxed), 2, "once a day");
}

#[tokio::test(start_paused = true)]
async fn a_release_out_since_is_found_a_day_later() {
    let mut copy = Running::start(
        UpdatedBy::Itself,
        FakeReleaseRepository::default(),
        Preferences::default(),
    );
    assert_eq!(copy.says_within(HOUR).await, None, "no release yet");

    copy.releases.releases(NEXT);

    assert_eq!(
        copy.says_within(DAY).await,
        Some(UpdateEvent::Installed(NEXT))
    );
}

#[tokio::test(start_paused = true)]
async fn each_release_is_told_of_once() {
    let mut copy = Running::start(
        UpdatedBy::Cargo,
        FakeReleaseRepository::with(NEXT),
        Preferences::default(),
    );
    assert!(copy.says_within(HOUR).await.is_some());

    assert_eq!(copy.says_within(3 * DAY).await, None);
}

#[tokio::test(start_paused = true)]
async fn a_release_it_cant_put_in_place_is_told_of_instead() {
    let releases = FakeReleaseRepository::with(NEXT);
    releases.cant_install.store(true, Ordering::Relaxed);
    let mut copy = Running::start(UpdatedBy::Itself, releases, Preferences::default());

    assert_eq!(
        copy.says_within(HOUR).await,
        Some(UpdateEvent::Available {
            version: NEXT,
            by: UpdatedBy::Itself
        }),
        "the install line gets it"
    );
}

#[tokio::test(start_paused = true)]
async fn turned_off_it_asks_nobody_anything() {
    let mut copy = Running::start(
        UpdatedBy::Itself,
        FakeReleaseRepository::with(NEXT),
        Preferences {
            auto_update: false,
            ..Default::default()
        },
    );

    assert_eq!(copy.says_within(3 * DAY).await, None);
    assert_eq!(copy.releases.asks.load(Ordering::Relaxed), 0);
}
