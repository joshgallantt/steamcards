use std::sync::Mutex;

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use crate::{EventKind, FarmingEvent, FarmingStatus, Status};

/// Sends the farmer's log lines and status to the UI, in order, and
/// remembers the last status so an interim "reading your badges…" keeps the
/// library on screen.
pub(crate) struct Reporter {
    tx: mpsc::Sender<FarmingEvent>,
    last: Mutex<FarmingStatus>,
}

impl Reporter {
    pub(crate) fn new(tx: mpsc::Sender<FarmingEvent>) -> Self {
        Self {
            tx,
            last: Mutex::default(),
        }
    }

    // try_send keeps events in order without blocking the farmer; the UI
    // drains continuously, so the buffer only fills if it has gone away.
    fn send(&self, kind: EventKind, message: String, status: Option<FarmingStatus>) {
        let _ = self.tx.try_send(FarmingEvent {
            kind,
            message,
            status,
        });
    }

    pub(crate) fn event(&self, kind: EventKind, message: String) {
        self.send(kind, message, None);
    }

    pub(crate) fn info(&self, message: String) {
        self.event(EventKind::Info, message);
    }

    pub(crate) fn status(&self, s: FarmingStatus) {
        *self.last.lock().unwrap() = s.clone();
        self.send(EventKind::Info, String::new(), Some(s));
    }

    /// Status with a note, keeping the last library and order on screen.
    pub(crate) fn stage(&self, status: Status, note: &str) {
        self.staged(status, note, None);
    }

    /// Status with a note, and when the farmer tries again: after an error,
    /// the screens say so.
    pub(crate) fn stage_until(&self, status: Status, note: &str, again: DateTime<Utc>) {
        self.staged(status, note, Some(again));
    }

    fn staged(&self, status: Status, note: &str, next_look: Option<DateTime<Utc>>) {
        let last = self.last.lock().unwrap().clone();
        self.status(FarmingStatus {
            status,
            note: note.to_owned(),
            playing: Vec::new(),
            mode: None,
            blocked_by: None,
            next_look,
            look_every: None,
            ..last
        });
    }

    /// Changes the last status without sending it: what the next status
    /// keeps.
    pub(crate) fn remember(&self, change: impl FnOnce(&mut FarmingStatus)) {
        change(&mut self.last.lock().unwrap());
    }

    /// The last status again, with what changed.
    pub(crate) fn amend(&self, change: impl FnOnce(&mut FarmingStatus)) {
        let mut last = self.last.lock().unwrap().clone();
        change(&mut last);
        self.status(last);
    }
}
