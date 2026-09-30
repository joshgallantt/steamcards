use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Serialize, de::DeserializeOwned};

use crate::private_file::write_whole;

/// A JSON file of its own, holding one `T`: read once when opened, and
/// written whole. It's for what can be lost, like a cache: a file that's
/// missing, or doesn't read, holds `T`'s default, and the next save writes
/// it afresh.
pub struct JsonFile<T> {
    path: PathBuf,
    value: Mutex<T>,
}

impl<T: Serialize + DeserializeOwned + Default + Clone> JsonFile<T> {
    /// Reads the file at `path`: the default when there's none, or it
    /// doesn't read.
    pub fn open(path: PathBuf) -> Self {
        let value = fs::read(&path)
            .ok()
            .and_then(|data| serde_json::from_slice(&data).ok())
            .unwrap_or_default();
        Self {
            path,
            value: Mutex::new(value),
        }
    }

    pub fn get(&self) -> T {
        self.value.lock().unwrap().clone()
    }

    /// Keeps `value` in place of what was there. Writes first, then keeps: a
    /// failed write leaves what was there.
    pub fn save(&self, value: T) -> anyhow::Result<()> {
        let mut kept = self.value.lock().unwrap();
        write_whole(&self.path, &serde_json::to_vec(&value)?)?;
        *kept = value;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("steamcards-json-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("prices.json")
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
    struct Sets {
        #[serde(default)]
        sets: Vec<u32>,
    }

    fn sets() -> Sets {
        Sets {
            sets: vec![960_910, 620],
        }
    }

    #[test]
    fn what_is_saved_is_read_back_after_a_restart() {
        let path = temp("roundtrip");
        let file = JsonFile::<Sets>::open(path.clone());
        assert_eq!(file.get(), Sets::default(), "no file yet");

        file.save(sets()).unwrap();

        assert_eq!(JsonFile::<Sets>::open(path).get(), sets());
    }

    #[test]
    fn a_file_that_doesnt_read_holds_the_default() {
        let path = temp("garbled");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "{\"sets\": [9").unwrap();

        let file = JsonFile::<Sets>::open(path.clone());

        assert_eq!(file.get(), Sets::default());
        file.save(sets()).unwrap();
        assert_eq!(JsonFile::<Sets>::open(path).get(), sets(), "written afresh");
    }

    #[test]
    fn a_failed_write_keeps_what_was_there() {
        // A directory where the file should be makes the rename fail.
        let path = temp("unwritable");
        fs::create_dir_all(&path).unwrap();
        let file = JsonFile::<Sets>::open(path);
        assert!(file.save(sets()).is_err());
        assert_eq!(file.get(), Sets::default());
    }

    #[cfg(unix)]
    #[test]
    fn only_its_owner_can_read_it() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp("private");
        JsonFile::<Sets>::open(path.clone()).save(sets()).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
