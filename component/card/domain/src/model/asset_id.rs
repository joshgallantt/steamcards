use std::fmt;

/// An item's identity: its ID in the account's Steam inventory. Each copy of
/// a card is an item of its own, so two copies are two asset IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssetId(pub u64);

impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
