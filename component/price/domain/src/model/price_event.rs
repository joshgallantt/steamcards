use crate::PriceEventKind;

/// Something the price watcher did, for the log.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceEvent {
    pub kind: PriceEventKind,
    /// What happened, in the user's words.
    pub message: String,
}
