use std::fmt;

use crate::MarketPause;

/// Why something asked of the market didn't happen, in the user's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceError {
    /// A change didn't stick: the settings couldn't be saved.
    Unavailable,
    /// Steam has paused price lookups until the pause ends.
    Paused(MarketPause),
    /// The market couldn't be asked just now: no sign-in, or no network.
    Unanswered,
}

impl fmt::Display for PriceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PriceError::Unavailable => write!(f, "the market settings couldn't be saved"),
            PriceError::Paused(_) => write!(f, "Steam has paused price lookups"),
            PriceError::Unanswered => write!(f, "the market couldn't be asked just now"),
        }
    }
}

impl std::error::Error for PriceError {}
