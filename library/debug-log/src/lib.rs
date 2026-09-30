//! Opt-in debug logging to a file, or to standard error when nothing owns the
//! screen. The TUI does own it, so stdout and stderr are not usable while it
//! runs.
//!
//! A value, not a global: the composition root decides where lines go and
//! hands a `DebugLog` to whatever may write one. Nothing here reads the
//! environment.

use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    path::Path,
    sync::{Arc, Mutex},
};

/// Where debug lines go, if anywhere. Cheap to clone; clones share the
/// destination.
#[derive(Clone, Default)]
pub struct DebugLog {
    sink: Option<Arc<Mutex<Box<dyn Write + Send>>>>,
    tag: &'static str,
}

impl DebugLog {
    /// Drops every line.
    pub fn off() -> Self {
        Self::default()
    }

    /// Appends to `path`, readable by its owner alone: the log holds what
    /// Steam said about the account. A file that can't be opened turns
    /// logging off rather than stopping the app.
    pub fn to_file(path: &Path) -> Self {
        open_private(path).map_or_else(|_| Self::off(), Self::to)
    }

    /// Standard error, on every OS. Only for when nothing owns the screen,
    /// like headless mode.
    pub fn to_stderr() -> Self {
        Self::to(io::stderr())
    }

    fn to(sink: impl Write + Send + 'static) -> Self {
        Self {
            sink: Some(Arc::new(Mutex::new(Box::new(sink)))),
            tag: "",
        }
    }

    /// The same destination, with every line prefixed by `tag`.
    pub fn tagged(&self, tag: &'static str) -> Self {
        Self {
            sink: self.sink.clone(),
            tag,
        }
    }

    /// Appends `<tag> <timestamp> <msg>` when logging is on.
    pub fn line(&self, msg: &str) {
        let Some(sink) = &self.sink else { return };
        if let Ok(mut out) = sink.lock() {
            let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(out, "{} {ts} {msg}", self.tag);
        }
    }
}

/// Opens `path` to append to, readable and writable by its owner alone; one
/// made before, with looser permissions, is tightened.
#[cfg(unix)]
fn open_private(path: &Path) -> io::Result<File> {
    use std::{
        fs::Permissions,
        os::unix::fs::{OpenOptionsExt, PermissionsExt},
    };

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    file.set_permissions(Permissions::from_mode(0o600))?;
    Ok(file)
}

/// Opens `path` to append to. Elsewhere than macOS and Linux, the folder's
/// own permissions decide who can read it.
#[cfg(not(unix))]
fn open_private(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tagged_lines_share_one_file() {
        let path =
            std::env::temp_dir().join(format!("steamcards-debug-{}.log", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let log = DebugLog::to_file(&path);
        log.tagged("cm").line("hello");
        log.tagged("web").line("there");
        DebugLog::off().line("nowhere");

        let text = std::fs::read_to_string(&path).unwrap();
        let tags: Vec<_> = text.lines().map(|l| l.split(' ').next().unwrap()).collect();
        assert_eq!(tags, ["cm", "web"]);
        assert!(text.contains(" hello") && text.contains(" there"));
    }

    /// Collects what's written, to look at afterwards.
    #[derive(Clone, Default)]
    struct Captured(Arc<Mutex<Vec<u8>>>);

    impl Write for Captured {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn any_stream_can_take_the_lines() {
        let captured = Captured::default();
        DebugLog::to(captured.clone()).tagged("web").line("hello");

        let text = String::from_utf8(captured.0.lock().unwrap().clone()).unwrap();
        assert!(
            text.starts_with("web ") && text.ends_with(" hello\n"),
            "{text:?}"
        );
    }

    #[test]
    fn a_file_that_cant_be_opened_turns_logging_off() {
        let nowhere = std::env::temp_dir()
            .join("no-such-dir-for-steamcards")
            .join("x.log");
        let log = DebugLog::to_file(&nowhere);
        assert!(log.sink.is_none());
        log.line("dropped quietly");
    }

    #[cfg(unix)]
    #[test]
    fn only_its_owner_can_read_the_file() {
        use std::os::unix::fs::PermissionsExt;

        let path =
            std::env::temp_dir().join(format!("steamcards-private-{}.log", std::process::id()));
        std::fs::write(&path, "from before\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        DebugLog::to_file(&path).line("hello");

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "a log from before is tightened too");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("from before\n")
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn standard_error_is_a_destination_on_every_os() {
        assert!(DebugLog::to_stderr().sink.is_some());
    }
}
