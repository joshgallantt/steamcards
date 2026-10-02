//! Everything the process is told from outside, read once, here.
//!
//! Nothing else in the workspace reads the environment or asks the OS for its
//! directories (clippy's `disallowed_methods` stops it; see clippy.toml). Each
//! setting reaches the code that uses it as an ordinary argument, through the
//! composition root.

#![expect(clippy::disallowed_methods, reason = "the one place settings are read")]

use std::path::PathBuf;

use anyhow::anyhow;
use debug_log::DebugLog;

pub(crate) struct Settings {
    /// `$STEAMCARDS_CONFIG`, or `<config-dir>/steamcards/config.json`, where
    /// `<config-dir>` is this computer's own: on Windows, `%LOCALAPPDATA%`.
    pub config_path: PathBuf,
    /// The market's prices, in a file of their own beside the config file:
    /// `prices.json`.
    pub prices_path: PathBuf,
    pub debug_log: DebugLog,
    /// The running copy, links followed: where an update goes.
    pub exe: PathBuf,
    /// Where cargo builds to: `$CARGO_HOME`, or `~/.cargo`. A copy there is
    /// cargo's to update.
    pub cargo_home: PathBuf,
    /// Where releases are: `$STEAMCARDS_RELEASES_URL`, as the install
    /// scripts take it, or GitHub's releases page.
    pub releases: String,
}

impl Settings {
    /// `headless` sends debug lines to stderr when `$STEAMCARDS_DEBUG` is
    /// unset, since nothing owns the screen.
    pub(crate) fn from_environment(headless: bool) -> anyhow::Result<Self> {
        // This computer's folder for settings. On Windows that's the local
        // one, not the roaming one that follows a user to other PCs: the
        // sign-in is this computer's session with Steam, and two PCs sharing
        // it would knock each other off. On macOS and Linux they're the same.
        let config_dir = || {
            dirs::config_local_dir()
                .map(|d| d.join("steamcards"))
                .ok_or_else(|| anyhow!("could not determine config directory"))
        };

        let config_path = match std::env::var_os("STEAMCARDS_CONFIG") {
            Some(p) => PathBuf::from(p),
            None => config_dir()?.join("config.json"),
        };

        // "1" or "true" logs beside the config; anything else is a path. On a
        // first run the config's folder isn't there yet: make it, or the log
        // couldn't open.
        let debug_log = match std::env::var("STEAMCARDS_DEBUG").ok().as_deref() {
            Some("1" | "true") => {
                let dir = config_dir()?;
                std::fs::create_dir_all(&dir)?;
                DebugLog::to_file(&dir.join("debug.log"))
            }
            Some(path) => DebugLog::to_file(path.as_ref()),
            None if headless => DebugLog::to_stderr(),
            None => DebugLog::off(),
        };

        let exe = std::env::current_exe()
            .and_then(|p| p.canonicalize())
            .unwrap_or_default();
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|home| home.join(".cargo")))
            .unwrap_or_default();
        let releases = std::env::var("STEAMCARDS_RELEASES_URL")
            .unwrap_or_else(|_| concat!(env!("CARGO_PKG_REPOSITORY"), "/releases").to_owned());

        Ok(Self {
            prices_path: config_path.with_file_name("prices.json"),
            config_path,
            debug_log,
            exe,
            cargo_home,
            releases,
        })
    }
}
