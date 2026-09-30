use chrono::{DateTime, Utc};

/// A game whose last drop came this session: seen at `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finished {
    pub app_id: u32,
    pub at: DateTime<Utc>,
}
