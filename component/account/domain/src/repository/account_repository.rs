use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::LoginChallenge;

/// The saved Steam sign-in. Declared here, beside the use cases that need it;
/// the data layer is written to fit.
#[async_trait]
pub trait AccountRepository: Send + Sync {
    /// A sign-in is saved.
    fn is_linked(&self) -> bool;

    /// The account's sign-in name, if Steam said it.
    fn name(&self) -> Option<String>;

    /// True once Steam has rejected the saved sign-in. Cached — call `verify`
    /// to refresh it.
    fn is_rejected(&self) -> bool;

    /// Checks whether the saved sign-in still works, and updates
    /// `is_rejected`. A check that couldn't reach Steam leaves the previous
    /// answer in place.
    async fn verify(&self);

    /// Signs in with the Steam app. Sends a `LoginChallenge` on `challenges`
    /// whenever there's something new to show, and returns once the sign-in
    /// is saved. Errs with a reason fit to show the user.
    async fn link(&self, challenges: mpsc::UnboundedSender<LoginChallenge>) -> anyhow::Result<()>;

    /// Forgets the saved sign-in. Errs, keeping it, when it couldn't be
    /// forgotten. Telling Steam to end it happens in the background and
    /// doesn't hold this up.
    fn unlink(&self) -> anyhow::Result<()>;
}
