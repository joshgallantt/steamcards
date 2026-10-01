use crate::{FarmingEvent, FarmingStatus};

/// What the farmer tells the screens, in order: where farming stands, or
/// something that happened. A status, the whole library and session, goes
/// boxed: events are many and small.
#[derive(Debug, Clone, PartialEq)]
pub enum FarmingUpdate {
    Status(Box<FarmingStatus>),
    Event(FarmingEvent),
}
