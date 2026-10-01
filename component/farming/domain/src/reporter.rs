use std::sync::Mutex;

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use crate::{FarmingEvent, FarmingStatus, FarmingUpdate, Status, Trouble};

/// Sends what happened and where farming stands to the screens, in order,
/// and remembers the last status so an interim one while the badges are
/// read keeps the library on screen.
pub(crate) struct Reporter {
    tx: mpsc::Sender<FarmingUpdate>,
    last: Mutex<FarmingStatus>,
}

impl Reporter {
    pub(crate) fn new(tx: mpsc::Sender<FarmingUpdate>) -> Self {
        Self {
            tx,
            last: Mutex::default(),
        }
    }

    // try_send keeps updates in order without blocking the farmer; the UI
    // drains continuously, so the buffer only fills if it has gone away.
    fn send(&self, update: FarmingUpdate) {
        let _ = self.tx.try_send(update);
    }

    pub(crate) fn event(&self, event: FarmingEvent) {
        self.send(FarmingUpdate::Event(event));
    }

    pub(crate) fn status(&self, s: FarmingStatus) {
        *self.last.lock().unwrap() = s.clone();
        self.send(FarmingUpdate::Status(Box::new(s)));
    }

    /// A status between spells of playing, keeping the last library and
    /// order on screen.
    pub(crate) fn stage(&self, status: Status) {
        self.staged(status, None, None);
    }

    /// An error's status: what went wrong, and when the farmer tries again,
    /// if it does.
    pub(crate) fn stage_trouble(&self, trouble: Trouble, again: Option<DateTime<Utc>>) {
        self.staged(Status::Error, Some(trouble), again);
    }

    fn staged(&self, status: Status, trouble: Option<Trouble>, next_look: Option<DateTime<Utc>>) {
        let last = self.last.lock().unwrap().clone();
        self.status(FarmingStatus {
            status,
            playing: Vec::new(),
            mode: None,
            blocked_by: None,
            next_look,
            look_every: None,
            nothing_to_farm: None,
            trouble,
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
