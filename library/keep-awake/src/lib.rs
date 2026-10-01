//! Keeping the computer awake while games are being played: Steam counts a
//! game as played only while its session is awake to say so.
//!
//! The system's own tool does it: `caffeinate` on macOS, and `systemd-inhibit`
//! on Linux with systemd. Either holds the machine awake for as long as it
//! runs, and both are told to stop when steamcards does, so a crash can't
//! leave the machine unable to sleep. The display may still sleep. Without the
//! tool, nothing happens.

use std::{
    process::{Child, Command, Stdio},
    sync::Mutex,
};

use debug_log::DebugLog;

pub struct KeepAwake {
    /// The program that holds the machine awake while it runs, and its
    /// arguments; `None` does nothing.
    holder: Option<(String, Vec<String>)>,
    held: Mutex<Option<Child>>,
    log: DebugLog,
}

impl KeepAwake {
    /// The system's own way to stay awake, until this process ends.
    pub fn system(log: &DebugLog) -> Self {
        let pid = std::process::id();
        let holder = if cfg!(target_os = "macos") {
            // No idle, disk-idle or (on power) system sleep, until steamcards exits.
            Some((
                "caffeinate",
                vec!["-ims".to_owned(), "-w".to_owned(), pid.to_string()],
            ))
        } else if cfg!(target_os = "linux") {
            let args = [
                "--what=idle:sleep",
                "--who=steamcards",
                "--why=Farming Steam trading cards",
                "--mode=block",
                "tail",
            ]
            .map(str::to_owned)
            .into_iter()
            .chain([
                format!("--pid={pid}"),
                "-f".to_owned(),
                "/dev/null".to_owned(),
            ])
            .collect();
            Some(("systemd-inhibit", args))
        } else {
            None
        };
        Self {
            holder: holder.map(|(program, args)| (program.to_owned(), args)),
            held: Mutex::new(None),
            log: log.tagged("awake"),
        }
    }

    /// Keeps nothing awake.
    pub fn off() -> Self {
        Self {
            holder: None,
            held: Mutex::new(None),
            log: DebugLog::off(),
        }
    }

    /// Runs `program` with `args` to hold the machine awake: for tests,
    /// something harmless like `sleep`.
    pub fn running(program: &str, args: &[&str]) -> Self {
        Self {
            holder: Some((
                program.to_owned(),
                args.iter().map(|a| (*a).to_owned()).collect(),
            )),
            held: Mutex::new(None),
            log: DebugLog::off(),
        }
    }

    /// Holds the machine awake, if it isn't already, until `let_sleep`.
    pub fn hold(&self) {
        let Some((program, args)) = &self.holder else {
            return;
        };
        let mut held = self.held.lock().unwrap();
        if let Some(child) = held.as_mut()
            && matches!(child.try_wait(), Ok(None))
        {
            return;
        }
        match Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                self.log.line(&format!("staying awake ({program})"));
                *held = Some(child);
            }
            Err(e) => self
                .log
                .line(&format!("couldn't stay awake with {program}: {e}")),
        }
    }

    /// Lets the machine sleep again.
    pub fn let_sleep(&self) {
        if let Some(mut child) = self.held.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
            self.log.line("letting the computer sleep");
        }
    }

    /// Whether the machine is being held awake.
    #[cfg(any(test, feature = "test-support"))]
    pub fn is_held(&self) -> bool {
        self.held
            .lock()
            .unwrap()
            .as_mut()
            .is_some_and(|c| matches!(c.try_wait(), Ok(None)))
    }
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        self.let_sleep();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holding_runs_the_holder_once_and_letting_go_stops_it() {
        let awake = KeepAwake::running("sleep", &["60"]);
        assert!(!awake.is_held());
        awake.hold();
        assert!(awake.is_held());
        let first = awake.held.lock().unwrap().as_ref().map(Child::id);
        awake.hold();
        let second = awake.held.lock().unwrap().as_ref().map(Child::id);
        assert_eq!(first, second, "one holder at a time");
        awake.let_sleep();
        assert!(!awake.is_held());
    }

    #[test]
    fn a_holder_that_isnt_there_changes_nothing() {
        let awake = KeepAwake::running("no-such-program-for-steamcards", &[]);
        awake.hold();
        assert!(!awake.is_held());
        KeepAwake::off().hold();
    }

    #[test]
    fn dropping_it_lets_go() {
        let awake = KeepAwake::running("sleep", &["60"]);
        awake.hold();
        let pid = awake.held.lock().unwrap().as_ref().map(Child::id).unwrap();
        drop(awake);
        let alive = Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .is_ok_and(|s| s.success());
        assert!(!alive, "the holder stopped");
    }
}
