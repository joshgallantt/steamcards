use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};

use async_trait::async_trait;

use crate::{ReleaseRepository, Version};

/// The releases, held in memory: the latest, and every one put in place.
#[derive(Default)]
pub struct FakeReleaseRepository {
    /// The latest release; `None` while there's none.
    pub latest: Mutex<Option<Version>>,
    /// How often the latest was asked for.
    pub asks: AtomicUsize,
    /// Every release put in place, in order.
    pub installed: Mutex<Vec<Version>>,
    /// Putting one in place fails, as if the folder couldn't be written.
    pub cant_install: AtomicBool,
}

impl FakeReleaseRepository {
    /// With `latest` the latest release.
    pub fn with(latest: Version) -> Self {
        let repo = Self::default();
        repo.releases(latest);
        repo
    }

    /// A new release comes out.
    pub fn releases(&self, version: Version) {
        *self.latest.lock().unwrap() = Some(version);
    }
}

#[async_trait]
impl ReleaseRepository for FakeReleaseRepository {
    async fn latest(&self) -> anyhow::Result<Option<Version>> {
        self.asks.fetch_add(1, Ordering::Relaxed);
        Ok(*self.latest.lock().unwrap())
    }

    async fn install(&self, version: Version) -> anyhow::Result<()> {
        if self.cant_install.load(Ordering::Relaxed) {
            anyhow::bail!("its folder can't be written to");
        }
        self.installed.lock().unwrap().push(version);
        Ok(())
    }
}
