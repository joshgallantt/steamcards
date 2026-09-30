/// What signing in needs from the user: scan a QR code with the Steam app,
/// then approve it there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginChallenge {
    /// What the QR code holds. A new one replaces the last.
    pub url: String,
    /// The Steam app has scanned it: approving it there is all that's left.
    pub scanned: bool,
}
