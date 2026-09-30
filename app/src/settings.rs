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
    /// `$STEAMCARDS_CONFIG`, or `<config-dir>/steamcards/config.json`.
    pub config_path: PathBuf,
    /// The market's prices, in a file of their own beside the config file:
    /// `prices.json`.
    pub prices_path: PathBuf,
    pub debug_log: DebugLog,
}

impl Settings {
    /// `headless` sends debug lines to stderr when `$STEAMCARDS_DEBUG` is
    /// unset, since nothing owns the screen.
    pub(crate) fn from_environment(headless: bool) -> anyhow::Result<Self> {
        let config_dir = || {
            dirs::config_dir()
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

        Ok(Self {
            prices_path: config_path.with_file_name("prices.json"),
            config_path,
            debug_log,
        })
    }
}
