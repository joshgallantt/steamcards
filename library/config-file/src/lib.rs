//! The JSON files steamcards keeps: the config file, with the saved sign-in,
//! the preferences and the market's settings, and beside it, the market's
//! prices (see [`PriceCache`]). Where they live is the composition root's
//! decision; this crate opens the paths it is given.
//!
//! Storage only: it knows the files' shape, not what any of it means. What a
//! valid sign-in is belongs to the Steam client; what a preference means
//! belongs to the `preferences` component, and a price to `market`.
//!
//! The config file holds a sign-in, so on macOS and Linux only its owner can
//! read it.

mod prices;

use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

pub use prices::{PriceCache, StoredCard, StoredPrice, StoredSet};

/// The saved Steam sign-in, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Credentials {
    /// What Steam handed over when the sign-in was approved. It signs in to
    /// Steam, and makes the tokens steamcommunity.com takes.
    pub refresh_token: String,
    /// The account's sign-in name.
    pub account_name: String,
    /// The account's 64-bit Steam ID.
    pub steam_id: u64,
    /// A random number Steam tells this computer's sessions apart by. Kept
    /// across sign-ins, so signing in again doesn't look like a new device.
    pub login_id: u32,
}

/// Where the saved sign-in lives. The Steam client depends on this, not on
/// the file.
pub trait CredentialStore: Send + Sync {
    /// The saved sign-in, or `None` when there is no token.
    fn credentials(&self) -> Option<Credentials>;
    fn save_credentials(&self, c: Credentials) -> anyhow::Result<()>;
    /// Forgets the saved sign-in. The rest of the file is kept.
    fn forget_credentials(&self) -> anyhow::Result<()>;
}

/// Preferences, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredPreferences {
    pub priority_games: Vec<u32>,
    pub skipped_games: Vec<u32>,
    pub only_priority: bool,
    pub appear_online: bool,
}

/// The market's settings, and Steam's pause on market requests, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StoredMarket {
    /// The value basis: "list", "net" or "instant". Empty for the default.
    pub basis: String,
    /// Steam's pause on market requests, when there is one.
    pub pause: Option<StoredPause>,
}

/// Steam's pause on market requests, as stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredPause {
    /// When it ends, in seconds since the epoch.
    pub until: i64,
    /// How long it is, in seconds.
    pub step: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct MarketDto {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pause: Option<StoredPause>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct SteamDto {
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    account_name: String,
    /// A string, not a number: Steam IDs are too big for a JSON number to
    /// hold exactly in most tools that might read the file.
    #[serde(default)]
    steam_id: String,
    #[serde(default)]
    login_id: u32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct FileDto {
    #[serde(default)]
    steam: SteamDto,
    #[serde(default)]
    priority_games: Vec<u32>,
    #[serde(default)]
    skipped_games: Vec<u32>,
    #[serde(default)]
    only_priority: bool,
    #[serde(default)]
    appear_online: bool,
    #[serde(default)]
    market: MarketDto,
}

pub struct ConfigFile {
    path: PathBuf,
    model: Mutex<FileDto>,
}

impl ConfigFile {
    /// Reads the file at `path`, or starts empty if there is none.
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        let model = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            FileDto::default()
        };
        Ok(Self {
            path,
            model: Mutex::new(model),
        })
    }

    pub fn preferences(&self) -> StoredPreferences {
        let m = self.model.lock().unwrap();
        StoredPreferences {
            priority_games: m.priority_games.clone(),
            skipped_games: m.skipped_games.clone(),
            only_priority: m.only_priority,
            appear_online: m.appear_online,
        }
    }

    /// Writes first, then keeps: a failed write leaves memory as it was, so
    /// nothing reports a change the file doesn't have.
    pub fn save_preferences(&self, p: StoredPreferences) -> anyhow::Result<()> {
        self.update(|m| {
            m.priority_games = p.priority_games;
            m.skipped_games = p.skipped_games;
            m.only_priority = p.only_priority;
            m.appear_online = p.appear_online;
        })
    }

    pub fn market(&self) -> StoredMarket {
        let m = self.model.lock().unwrap();
        StoredMarket {
            basis: m.market.basis.clone(),
            pause: m.market.pause,
        }
    }

    /// Writes first, then keeps, as the preferences are saved.
    pub fn save_market_basis(&self, basis: String) -> anyhow::Result<()> {
        self.update(|m| m.market.basis = basis)
    }

    /// Writes first, then keeps. `None` once there's no pause.
    pub fn save_market_pause(&self, pause: Option<StoredPause>) -> anyhow::Result<()> {
        self.update(|m| m.market.pause = pause)
    }

    fn update(&self, change: impl FnOnce(&mut FileDto)) -> anyhow::Result<()> {
        let mut model = self.model.lock().unwrap();
        let mut next = model.clone();
        change(&mut next);
        self.persist(&next)?;
        *model = next;
        Ok(())
    }

    fn persist(&self, model: &FileDto) -> anyhow::Result<()> {
        write_whole(&self.path, &serde_json::to_vec_pretty(model)?)
    }
}

/// Writes `data` as the whole of the file at `path`, or nothing: to a file
/// beside it first, then renamed over it.
fn write_whole(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    write_private(&tmp, data)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Writes `data` to `path`, readable and writable by its owner alone.
#[cfg(unix)]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};

    // A file left over from a failed write may have looser permissions.
    let _ = fs::remove_file(path);
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(data)
}

/// Writes `data` to `path`. Elsewhere than macOS and Linux, the folder's own
/// permissions decide who can read it.
#[cfg(not(unix))]
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    fs::write(path, data)
}

impl CredentialStore for ConfigFile {
    fn credentials(&self) -> Option<Credentials> {
        let model = self.model.lock().unwrap();
        let s = &model.steam;
        if s.refresh_token.is_empty() {
            return None;
        }
        Some(Credentials {
            refresh_token: s.refresh_token.clone(),
            account_name: s.account_name.clone(),
            steam_id: s.steam_id.parse().unwrap_or_default(),
            login_id: s.login_id,
        })
    }

    fn forget_credentials(&self) -> anyhow::Result<()> {
        self.update(|m| m.steam = SteamDto::default())
    }

    fn save_credentials(&self, c: Credentials) -> anyhow::Result<()> {
        self.update(|m| {
            m.steam = SteamDto {
                refresh_token: c.refresh_token,
                account_name: c.account_name,
                steam_id: c.steam_id.to_string(),
                login_id: c.login_id,
            };
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("steamcards-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("config.json")
    }

    fn signed_in() -> Credentials {
        Credentials {
            refresh_token: "eyJ.refresh.token".into(),
            account_name: "cardfarmer".into(),
            steam_id: 76_561_197_960_287_930,
            login_id: 1_234_567,
        }
    }

    #[test]
    fn what_is_saved_is_read_back_after_a_restart() {
        let path = temp("roundtrip");
        let file = ConfigFile::open(path.clone()).unwrap();
        file.save_credentials(signed_in()).unwrap();
        let prefs = StoredPreferences {
            priority_games: vec![620, 440],
            skipped_games: vec![730],
            only_priority: true,
            appear_online: true,
        };
        file.save_preferences(prefs.clone()).unwrap();

        let reopened = ConfigFile::open(path).unwrap();
        assert_eq!(reopened.credentials(), Some(signed_in()));
        assert_eq!(reopened.preferences(), prefs);
    }

    #[test]
    fn a_forgotten_sign_in_is_gone_after_a_restart_and_nothing_else_is() {
        let path = temp("forget");
        let file = ConfigFile::open(path.clone()).unwrap();
        file.save_credentials(signed_in()).unwrap();
        let prefs = StoredPreferences {
            priority_games: vec![620],
            ..Default::default()
        };
        file.save_preferences(prefs.clone()).unwrap();

        file.forget_credentials().unwrap();

        let reopened = ConfigFile::open(path).unwrap();
        assert_eq!(reopened.credentials(), None);
        assert_eq!(reopened.preferences(), prefs);
    }

    #[test]
    fn steam_ids_are_kept_whole() {
        let path = temp("format");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"{"steam":{"refresh_token":"t","account_name":"me","steam_id":"76561197960287930","login_id":7},"priority_games":[620],"only_priority":true}"#,
        )
        .unwrap();
        let file = ConfigFile::open(path).unwrap();
        let creds = file.credentials().unwrap();
        assert_eq!(creds.steam_id, 76_561_197_960_287_930);
        assert_eq!(creds.login_id, 7);
        assert_eq!(file.preferences().priority_games, [620]);
        assert!(file.preferences().only_priority);
    }

    #[test]
    fn the_market_settings_and_steams_pause_are_kept_with_the_rest() {
        let path = temp("market");
        let file = ConfigFile::open(path.clone()).unwrap();
        file.save_credentials(signed_in()).unwrap();
        file.save_market_basis("net".into()).unwrap();
        let pause = StoredPause {
            until: 1_790_712_345,
            step: 1_200,
        };
        file.save_market_pause(Some(pause)).unwrap();

        let reopened = ConfigFile::open(path.clone()).unwrap();
        assert_eq!(
            reopened.market(),
            StoredMarket {
                basis: "net".into(),
                pause: Some(pause),
            }
        );
        assert_eq!(
            reopened.credentials(),
            Some(signed_in()),
            "and nothing else changed"
        );
        let json: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(
            json["market"],
            serde_json::json!({"basis": "net", "pause": {"until": 1_790_712_345, "step": 1_200}})
        );

        reopened.save_market_pause(None).unwrap();
        assert_eq!(ConfigFile::open(path).unwrap().market().pause, None);
    }

    #[test]
    fn a_file_from_before_the_market_reads_with_its_defaults() {
        let path = temp("before-market");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"priority_games":[620]}"#).unwrap();
        assert_eq!(
            ConfigFile::open(path).unwrap().market(),
            StoredMarket::default()
        );
    }

    #[cfg(unix)]
    #[test]
    fn only_its_owner_can_read_it() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp("private");
        let file = ConfigFile::open(path.clone()).unwrap();
        file.save_credentials(signed_in()).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_failed_write_changes_nothing() {
        // A directory where the file should be makes the rename fail.
        let path = temp("unwritable");
        fs::create_dir_all(&path).unwrap();
        let file = ConfigFile {
            path,
            model: Mutex::new(FileDto::default()),
        };
        let prefs = StoredPreferences {
            priority_games: vec![620],
            ..Default::default()
        };
        assert!(file.save_preferences(prefs).is_err());
        assert!(file.preferences().priority_games.is_empty());
    }
}
