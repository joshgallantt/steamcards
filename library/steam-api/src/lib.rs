//! Steam, in Steam's own terms: a CM server connection, sign-in with a QR code
//! the Steam app approves, playing games, the pages of steamcommunity.com as
//! the account's owner sees them, and each game's card page, the items in the
//! account's inventory, and the account's wallet. The market's requests go
//! out through here too, one at a time, but `card-data` reads the market.
//!
//! No domain knowledge. The data crates map what this returns onto their
//! domains; this crate never sees a domain type.
//!
//! Written from what the Steam client, SteamKit and ASF do (see
//! docs/research/steam-card-farming.md and market-and-session.md); the
//! messages are Valve's own, as SteamDatabase publishes them.

pub mod auth;
pub mod badges;
pub mod cm;
mod community;
mod directory;
mod eresult;
pub mod inventory;
mod packet;
pub mod page;
mod proto;
mod steam_client;
pub mod token;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use community::{Reply, WebLogin};
pub use eresult::EResult;
pub use steam_client::SteamClient;

/// What steamcards calls itself to Steam: the device the Steam app asks to
/// approve, and the machine a session is on.
pub(crate) const DEVICE_NAME: &str = "steamcards";

/// Where Steam is. Tests point these at local stand-ins.
#[derive(Debug, Clone)]
pub struct Endpoints {
    /// Steam's Web API, which lists the CM servers.
    pub api: String,
    /// The community site, where the badge pages and the market are.
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
