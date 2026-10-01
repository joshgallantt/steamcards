use std::{
    fs::{self, File, TryLockError},
    io,
    path::Path,
};

/// One steamcards at a time to a config file. Two would sign on to Steam as
/// the same session and knock each other off, and each would write its own
/// copy of the file over the other's. It's a lock on a file beside the
/// config file (`config.lock` beside `config.json`), held until it's
/// dropped. The system lets it go however steamcards ends, so a crash never
/// leaves it held.
pub struct ConfigLock {
    _held: File,
}

impl ConfigLock {
    /// Takes the lock for the config file at `config_path`; `None` while
    /// another steamcards holds it.
    pub fn take(config_path: &Path) -> io::Result<Option<Self>> {
        if let Some(dir) = config_path.parent() {
            fs::create_dir_all(dir)?;
        }
        let file = File::options()
            .create(true)
            .write(true)
            .truncate(false)
            .open(config_path.with_extension("lock"))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _held: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("steamcards-lock-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("config.json")
    }

    #[test]
    fn a_config_file_is_used_by_one_steamcards_at_a_time() {
        let path = temp("one");
        let first = ConfigLock::take(&path).unwrap();
        assert!(first.is_some(), "free at first");
        assert!(
            ConfigLock::take(&path).unwrap().is_none(),
            "held by the first"
        );

        drop(first);

        assert!(
            ConfigLock::take(&path).unwrap().is_some(),
            "free again once the first lets go"
        );
    }

    #[test]
    fn another_config_file_has_a_lock_of_its_own() {
        let mine = temp("mine");
        let other = temp("other");
        let _held = ConfigLock::take(&mine).unwrap();
        assert!(ConfigLock::take(&other).unwrap().is_some());
    }
}
