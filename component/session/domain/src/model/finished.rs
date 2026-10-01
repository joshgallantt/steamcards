use chrono::{DateTime, Utc};
use steam_library::AppId;

/// A game whose last drop came this session: seen at `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finished {
    pub app_id: AppId,
    pub at: DateTime<Utc>,
}
