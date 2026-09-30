use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

use crate::{CredentialStore, Credentials, private_file::write_whole};

/// The config file: one JSON object, whose fields each part of steamcards
/// reads and writes in its own shape, leaving the others' alone.
pub struct ConfigFile {
    path: PathBuf,
    fields: Mutex<Map<String, Value>>,
}

impl ConfigFile {
    /// Reads the file at `path`, or starts empty if there is none.
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        let fields = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            Map::new()
        };
        Ok(Self {
            path,
            fields: Mutex::new(fields),
        })
    }

    /// The fields `T` names, as the file has them. Fields the file doesn't
    /// have take `T`'s defaults, as its serde attributes say; one that
    /// doesn't read as `T` says is an error.
    pub fn read<T: DeserializeOwned>(&self) -> anyhow::Result<T> {
        let fields = self.fields.lock().unwrap().clone();
        Ok(serde_json::from_value(Value::Object(fields))?)
    }

    /// Writes `T`'s fields in place of what they were, keeping every other
    /// field. Writes first, then keeps: a failed write leaves memory as it
    /// was, so nothing reports a change the file doesn't have.
    pub fn write<T: Serialize>(&self, part: &T) -> anyhow::Result<()> {
        let Value::Object(part) = serde_json::to_value(part)? else {
            anyhow::bail!("only named fields can be written to the config file");
        };
        let mut fields = self.fields.lock().unwrap();
        let mut next = fields.clone();
        next.extend(part);
        write_whole(&self.path, &serde_json::to_vec_pretty(&next)?)?;
        *fields = next;
        Ok(())
    }
}

/// The saved sign-in's fields in the file.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SteamFields {
    #[serde(default)]
    steam: SteamDto,
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

impl CredentialStore for ConfigFile {
    fn credentials(&self) -> Option<Credentials> {
        let s = self.read::<SteamFields>().ok()?.steam;
        if s.refresh_token.is_empty() {
            return None;
        }
        Some(Credentials {
            refresh_token: s.refresh_token,
            account_name: s.account_name,
            steam_id: s.steam_id.parse().unwrap_or_default(),
            login_id: s.login_id,
        })
    }

    fn forget_credentials(&self) -> anyhow::Result<()> {
        self.write(&SteamFields::default())
    }

    fn save_credentials(&self, c: Credentials) -> anyhow::Result<()> {
        self.write(&SteamFields {
            steam: SteamDto {
                refresh_token: c.refresh_token,
                account_name: c.account_name,
                steam_id: c.steam_id.to_string(),
                login_id: c.login_id,
            },
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

    /// Some part of steamcards' fields, as it would declare them.
    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Games {
        #[serde(default)]
        priority_games: Vec<u32>,
        #[serde(default)]
        only_priority: bool,
    }

    fn games() -> Games {
        Games {
            priority_games: vec![620, 440],
            only_priority: true,
        }
    }

    #[test]
    fn what_is_saved_is_read_back_after_a_restart() {
        let path = temp("roundtrip");
        let file = ConfigFile::open(path.clone()).unwrap();
        file.save_credentials(signed_in()).unwrap();
        file.write(&games()).unwrap();

        let reopened = ConfigFile::open(path).unwrap();
        assert_eq!(reopened.credentials(), Some(signed_in()));
        assert_eq!(reopened.read::<Games>().unwrap(), games());
    }

    #[test]
    fn fields_the_file_doesnt_have_take_their_defaults() {
        let file = ConfigFile::open(temp("empty")).unwrap();
        assert_eq!(file.read::<Games>().unwrap(), Games::default());
        assert_eq!(file.credentials(), None);
    }

    #[test]
    fn a_forgotten_sign_in_is_gone_after_a_restart_and_nothing_else_is() {
        let path = temp("forget");
        let file = ConfigFile::open(path.clone()).unwrap();
        file.save_credentials(signed_in()).unwrap();
        file.write(&games()).unwrap();

        file.forget_credentials().unwrap();

        let reopened = ConfigFile::open(path).unwrap();
        assert_eq!(reopened.credentials(), None);
        assert_eq!(reopened.read::<Games>().unwrap(), games());
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
        assert_eq!(
            file.read::<Games>().unwrap(),
            Games {
                priority_games: vec![620],
                only_priority: true,
            }
        );
    }

    #[test]
    fn a_field_that_doesnt_read_is_an_error() {
        let path = temp("wrong");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, r#"{"priority_games":"620"}"#).unwrap();
        assert!(ConfigFile::open(path).unwrap().read::<Games>().is_err());
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
            fields: Mutex::default(),
        };
        assert!(file.write(&games()).is_err());
        assert_eq!(file.read::<Games>().unwrap(), Games::default());
    }
}
