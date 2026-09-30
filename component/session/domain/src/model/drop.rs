use chrono::{DateTime, Utc};

use crate::DropCard;

/// One card that dropped. Each copy is its own drop: a look that finds two
/// new cards makes two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drop {
    /// When a look at the game, or a read of the library, found it.
    pub at: DateTime<Utc>,
    /// The game it dropped for.
    pub app_id: u32,
    pub card: DropCard,
    /// Which copy of that card (by name and border) the account then held,
    /// from its own counts: the set's for a normal card, its foil badge's
    /// for a foil. 1 the first, 2 or more a spare. `None` while that isn't
    /// known: the card isn't, or its counts couldn't be read.
    pub copy: Option<u32>,
}

impl Drop {
    /// A copy beyond the first of its card: a badge level takes one of each.
    /// Not while which copy it is isn't known.
    pub fn is_spare(&self) -> bool {
        self.copy.is_some_and(|copy| copy > 1)
    }
}
