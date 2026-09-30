/// The saved Steam sign-in, as stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Credentials {
    /// What Steam handed over when the sign-in was approved. It signs in to
    /// Steam, and makes the tokens steamcommunity.com takes.
    pub refresh_token: String,
    /// The account's sign-in name.
    pub account_name: String,
    /// The account's 64-bit Steam ID.
    pub steam_id: u64,
    /// A random number Steam tells this computer's sessions apart by. Kept
    /// across sign-ins, so signing in again doesn't look like a new device.
    pub login_id: u32,
}
