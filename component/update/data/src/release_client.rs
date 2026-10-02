use anyhow::{Context, anyhow};
use async_trait::async_trait;
use reqwest::{StatusCode, header::LOCATION, redirect::Policy};

/// GitHub's say on steamcards' releases.
#[async_trait]
pub trait ReleaseClient: Send + Sync {
    /// The latest release's tag, like `v0.1.3`; `None` while there's none.
    async fn latest_tag(&self) -> anyhow::Result<Option<String>>;

    /// One of a release's files.
    async fn download(&self, tag: &str, file: &str) -> anyhow::Result<Vec<u8>>;
}

/// The releases page on GitHub, or a mirror laid out the same way:
/// `<releases>/latest` redirects to `<releases>/tag/vX.Y.Z`, and a release's
/// files are in `<releases>/download/vX.Y.Z/`.
pub struct GitHubReleaseClient {
    releases: String,
    /// Asks without following redirects: where `latest` goes is the answer.
    asking: reqwest::Client,
    /// Downloads, following GitHub's redirects to where the files are kept.
    downloading: reqwest::Client,
}

impl GitHubReleaseClient {
    /// `releases` is the releases page, like
    /// `https://github.com/joshgallantt/steamcards/releases`.
    pub fn new(releases: &str) -> Self {
        let agent = concat!("steamcards/", env!("CARGO_PKG_VERSION"));
        let client = |redirects| {
            reqwest::Client::builder()
                .user_agent(agent)
                .redirect(redirects)
                .build()
                .unwrap_or_default()
        };
        Self {
            releases: releases.trim_end_matches('/').to_owned(),
            asking: client(Policy::none()),
            downloading: client(Policy::limited(10)),
        }
    }
}

#[async_trait]
impl ReleaseClient for GitHubReleaseClient {
    async fn latest_tag(&self) -> anyhow::Result<Option<String>> {
        let reply = self
            .asking
            .get(format!("{}/latest", self.releases))
            .send()
            .await
            .context("couldn't reach GitHub")?;
        // With no release yet, GitHub says there's no such page, or sends
        // the releases page.
        if reply.status() == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !reply.status().is_redirection() {
            return Err(anyhow!("GitHub answered {}", reply.status()));
        }
        let to = reply
            .headers()
            .get(LOCATION)
            .and_then(|l| l.to_str().ok())
            .unwrap_or_default();
        Ok(to
            .split_once("/tag/")
            .map(|(_, tag)| tag.trim_end_matches('/').to_owned())
            .filter(|tag| !tag.is_empty()))
    }

    async fn download(&self, tag: &str, file: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!("{}/download/{tag}/{file}", self.releases);
        let reply = self
            .downloading
            .get(&url)
            .send()
            .await
            .with_context(|| format!("couldn't download {file}"))?;
        if !reply.status().is_success() {
            return Err(anyhow!(
                "downloading {file}, GitHub answered {}",
                reply.status()
            ));
        }
        Ok(reply
            .bytes()
            .await
            .with_context(|| format!("couldn't download {file}"))?
            .to_vec())
    }
}
