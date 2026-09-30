use chrono::{DateTime, Utc};
use game::AppId;

use crate::Mode;

/// What was played, how, and when: from when play started to when it
/// stopped, for any reason (done, switched, blocked, the connection lost,
/// paused, stopped). Waiting for another device, and being paused, aren't
/// stretches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stretch {
    pub app_ids: Vec<AppId>,
    pub mode: Mode,
    pub from: DateTime<Utc>,
    /// `None` while it goes on.
    pub to: Option<DateTime<Utc>>,
}
