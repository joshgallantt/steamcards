use std::fmt;

/// Why the library couldn't be read, or games played, in the user's terms.
/// Steam's reason is theirs to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    Unavailable(String),
    /// Another session signed on in this one's place, with this sign-in, as
    /// the library was read.
    Replaced,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::Unavailable(why) => write!(f, "{why}"),
            GameError::Replaced => write!(f, "another session signed on in this one's place"),
        }
    }
}

impl std::error::Error for GameError {}
