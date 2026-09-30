use std::fmt;

/// Why the library couldn't be read, in the user's terms. Steam's reason is
/// theirs to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameError {
    Unavailable(String),
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::Unavailable(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for GameError {}
