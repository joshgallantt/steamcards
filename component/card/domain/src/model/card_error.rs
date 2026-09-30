use std::fmt;

/// Why a game's cards, or the account's items, couldn't be looked at, in the
/// user's terms. Steam's reason is theirs to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardError {
    Unavailable(String),
}

impl fmt::Display for CardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CardError::Unavailable(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for CardError {}
