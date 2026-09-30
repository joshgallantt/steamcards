use std::fmt;

/// What the farmer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    /// Reading the library, or connecting.
    Checking,
    Farming,
    /// Another device is playing on the account; farming waits for it.
    Blocked,
    Error,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Idle => write!(f, "idle"),
            Status::Checking => write!(f, "checking"),
            Status::Farming => write!(f, "farming"),
            Status::Blocked => write!(f, "blocked"),
            Status::Error => write!(f, "error"),
        }
    }
}
