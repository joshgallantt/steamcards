use crate::{EventKind, FarmingStatus};

#[derive(Debug, Clone, PartialEq)]
pub struct FarmingEvent {
    pub kind: EventKind,
    pub message: String,
    pub status: Option<FarmingStatus>,
}
