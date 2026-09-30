use chrono::{DateTime, Utc};

/// A game put behind the others after 10 hours without a drop: how often,
/// and when last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAside {
    pub app_id: u32,
    pub times: u8,
    pub since: DateTime<Utc>,
}
