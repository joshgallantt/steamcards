use std::{path::PathBuf, sync::Arc};

use anyhow::{Context, anyhow};
use async_trait::async_trait;
use debug_log::DebugLog;
use update::{ReleaseRepository, Version};

use crate::{Asset, ReleaseClient, checksums, swap};

/// The releases as the client finds them, put in place of the copy at
/// `exe`.
pub struct DefaultReleaseRepository {
    client: Arc<dyn ReleaseClient>,
    exe: PathBuf,
    /// This computer's archive and program; `None` where there's no build.
    asset: Option<Asset>,
    log: DebugLog,
}

impl DefaultReleaseRepository {
    pub fn new(
        client: Arc<dyn ReleaseClient>,
        exe: PathBuf,
        asset: Option<Asset>,
        log: DebugLog,
    ) -> Self {
        Self {
            client,
            exe,
            asset,
            log,
        }
    }

    /// Downloads the release's archive and its checksums, checks the one
    /// against the other, and puts the program in it in place.
    async fn put_in_place(&self, version: Version) -> anyhow::Result<()> {
        let asset = self
            .asset
            .as_ref()
            .ok_or_else(|| anyhow!("there's no build of steamcards for this computer"))?;
        let tag = format!("v{version}");
        let archive = self.client.download(&tag, &asset.archive).await?;
        let sums = self.client.download(&tag, "SHA256SUMS").await?;
        checksums::check(&archive, &asset.archive, &String::from_utf8_lossy(&sums))?;
        let program = asset.program_in(&archive)?;
        swap::put_in_place(&self.exe, &program)
            .with_context(|| format!("couldn't write {}", self.exe.display()))
    }
}

#[async_trait]
impl ReleaseRepository for DefaultReleaseRepository {
    async fn latest(&self) -> anyhow::Result<Option<Version>> {
        let tag = self
            .client
            .latest_tag()
            .await
            .inspect_err(|e| self.log.line(&format!("looking for a release: {e}")))?;
        Ok(tag.and_then(|t| t.strip_prefix('v').and_then(Version::parse)))
    }

    async fn install(&self, version: Version) -> anyhow::Result<()> {
        let put = self.put_in_place(version).await;
        match &put {
            Ok(()) => self.log.line(&format!(
                "put steamcards {version} in place of {}",
                self.exe.display()
            )),
            Err(e) => self
                .log
                .line(&format!("putting steamcards {version} in place: {e}")),
        }
        put
    }
}
