use std::fmt;

/// Why signing out didn't finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlinkError {
    /// The saved sign-in couldn't be forgotten. It's still there.
    Unavailable,
}

impl fmt::Display for UnlinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnlinkError::Unavailable => write!(f, "the sign-in couldn't be forgotten"),
        }
    }
}

impl std::error::Error for UnlinkError {}
