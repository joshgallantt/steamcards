use crate::Credentials;

/// Where the saved sign-in lives. The Steam client depends on this, not on
/// the file.
pub trait CredentialStore: Send + Sync {
    /// The saved sign-in, or `None` when there is no token.
    fn credentials(&self) -> Option<Credentials>;
    fn save_credentials(&self, c: Credentials) -> anyhow::Result<()>;
    /// Forgets the saved sign-in. The rest of the file is kept.
    fn forget_credentials(&self) -> anyhow::Result<()>;
}
