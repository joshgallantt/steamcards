use std::fmt;

/// Why the library couldn't be read, in the user's terms. Steam's reason is
/// theirs to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamLibraryError {
    Unavailable(String),
}

impl fmt::Display for SteamLibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SteamLibraryError::Unavailable(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for SteamLibraryError {}
