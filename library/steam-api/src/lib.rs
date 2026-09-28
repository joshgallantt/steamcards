//! Steam, in Steam's own terms: a CM server connection, sign-in with a QR code
//! the Steam app approves, playing games, and the badge pages on
//! steamcommunity.com.
//!
//! No domain knowledge. The `account` and `farming` data crates map what this
//! returns onto their domains; this crate never sees a domain type.
//!
//! Written from what the Steam client, SteamKit and ASF do (see
//! docs/research/steam-card-farming.md); the messages are Valve's own, as
//! SteamDatabase publishes them.

pub mod auth;
pub mod badges;
pub mod cm;
mod community;
mod directory;
mod eresult;
mod packet;
mod proto;
mod session;
pub mod token;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use eresult::EResult;
pub use session::Session;

/// What steamcards calls itself to Steam: the device the Steam app asks to
/// approve, and the machine a session is on.
pub(crate) const DEVICE_NAME: &str = "steamcards";

/// Where Steam is. Tests point these at local stand-ins.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// Steam's Web API, which lists the CM servers.
    pub api: String,
    /// The community site, where the badge pages are.
    pub community: String,
    /// A CM server to use, in place of asking the Web API for one.
    pub cm: Option<String>,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            api: "https://api.steampowered.com".into(),
            community: "https://steamcommunity.com".into(),
            cm: None,
        }
    }
}
