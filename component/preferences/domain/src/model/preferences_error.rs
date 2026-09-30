use std::fmt;

/// Why a change to the preferences didn't happen, in the user's terms. A disk
/// that won't write and a file that can't be renamed are one fact to them: the
/// change didn't stick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreferencesError {
    Unavailable,
}

impl fmt::Display for PreferencesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PreferencesError::Unavailable => write!(f, "preferences couldn't be saved"),
        }
    }
}

impl std::error::Error for PreferencesError {}
