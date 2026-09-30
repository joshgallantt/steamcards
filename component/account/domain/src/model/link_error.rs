use std::fmt;

/// Why signing in didn't finish.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkError {
    /// Steam, or the Steam app, said no. The reason is the user's to read.
    Refused(String),
}

impl fmt::Display for LinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkError::Refused(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for LinkError {}
