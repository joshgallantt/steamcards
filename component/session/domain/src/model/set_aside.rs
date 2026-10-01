use chrono::{DateTime, Utc};
use steam_library::AppId;

/// A game put behind the others after 10 hours without a drop: how often,
/// and when last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAside {
    pub app_id: AppId,
    pub times: u8,
    pub since: DateTime<Utc>,
}
