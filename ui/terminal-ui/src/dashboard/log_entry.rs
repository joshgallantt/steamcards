use std::time::Instant;

use chrono::{DateTime, FixedOffset};

use crate::dashboard::EventKind;

/// One line of the log: what was said, and when.
pub(crate) struct LogEntry {
    /// When it happened, in the time zone it's shown in.
    pub(crate) at: DateTime<FixedOffset>,
    pub(crate) received: Instant,
    pub(crate) kind: EventKind,
    pub(crate) text: String,
}
