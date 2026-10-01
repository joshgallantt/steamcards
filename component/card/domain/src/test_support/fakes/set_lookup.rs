use crate::CardKind;
use chrono::{DateTime, Utc};

/// A set lookup that reached the market, and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetLookup {
    pub app_id: u32,
    pub kind: CardKind,
    pub at: DateTime<Utc>,
}
