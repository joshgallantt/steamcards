use async_trait::async_trait;

use crate::Version;

/// Where steamcards' releases come from, and how one is put in place.
/// Declared here, beside the use case that needs it; the data layer is
/// written to fit.
#[async_trait]
pub trait ReleaseRepository: Send + Sync {
    /// The latest release's version; `None` while there's no release to be
    /// had. Errs with a reason when it couldn't be asked.
    async fn latest(&self) -> anyhow::Result<Option<Version>>;

    /// Downloads this release for this computer, checks it against the
    /// release's checksums, and puts it in place of the running copy, for
    /// the next start. Errs with a reason fit to show the user, leaving the
    /// running copy as it was.
    async fn install(&self, version: Version) -> anyhow::Result<()>;
}
