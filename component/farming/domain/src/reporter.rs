use std::sync::Mutex;

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
        let last = self.last.lock().unwrap().clone();
        self.send(
            EventKind::Info,
            String::new(),
            Some(FarmingStatus {
                status,
                note: note.to_owned(),
                playing: Vec::new(),
                mode: None,
                blocked_by: None,
                next_look: None,
                ..last
            }),
        );
    }
}
