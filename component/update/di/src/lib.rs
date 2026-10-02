//! Where the update domain meets its data layer: steamcards' releases, as
//! GitHub publishes them, put in place of the running copy through one
//! repository. The composition root names where the releases are, the
//! running copy and its version, and where cargo builds to, which with the
//! copy's place says who updates it.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use debug_log::DebugLog;
use preferences::GetPreferencesUseCase;
use update::{DefaultKeepUpToDateUseCase, KeepUpToDateUseCase, ReleaseRepository, Version};
use update_data::{Asset, DefaultReleaseRepository, GitHubReleaseClient, updated_by};

pub struct UpdateComponent {
    pub keep_up_to_date: Arc<dyn KeepUpToDateUseCase>,
}

impl UpdateComponent {
    /// `releases` is the releases page, `exe` the running copy, `running`
    /// its version, like `0.1.2`, and `cargo_home` where cargo builds to.
    pub fn new(
        releases: &str,
        exe: PathBuf,
        running: &str,
        cargo_home: &Path,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        log: DebugLog,
    ) -> Self {
        let by = updated_by(&exe, cargo_home);
        let repo: Arc<dyn ReleaseRepository> = Arc::new(DefaultReleaseRepository::new(
            Arc::new(GitHubReleaseClient::new(releases)),
            exe,
            Asset::for_this_computer(),
            log,
        ));
        // A version that doesn't read, as from a build of no release, is
        // older than any release.
        let running = Version::parse(running).unwrap_or(Version::new(0, 0, 0));
        Self {
            keep_up_to_date: Arc::new(DefaultKeepUpToDateUseCase::new(
                repo,
                get_preferences,
                running,
                by,
            )),
        }
    }
}
