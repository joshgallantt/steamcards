use std::fmt;

/// The signed-in Steam account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Its sign-in name; empty when Steam didn't say.
    pub name: String,
    /// A sign-in is saved but Steam no longer takes it: signing in again is
    /// the way on.
    pub expired: bool,
}

/// What signing in needs from the user: scan a QR code with the Steam app,
/// then approve it there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginChallenge {
    /// What the QR code holds. A new one replaces the last.
    pub url: String,
    /// The Steam app has scanned it: approving it there is all that's left.
    pub scanned: bool,
}

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
