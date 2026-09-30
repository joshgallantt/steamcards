//! The JSON files steamcards keeps: the config file, where each part of
//! steamcards keeps its own fields beside the others', and beside it, the
//! market's prices (see [`PriceCache`]). Where they live is the composition
//! root's decision; this crate opens the paths it is given.
//!
//! Storage only: it knows how a file is kept, not what's in it. Each data
//! crate declares the shape of its own fields; the saved sign-in's is here,
//! since the Steam client, a library, keeps it.
//!
//! The config file holds a sign-in, so on macOS and Linux only its owner can
//! read it.

mod config_file;
mod credential_store;
mod credentials;
mod prices;
mod private_file;

pub use config_file::{ConfigFile, StoredMarket, StoredPause};
pub use credential_store::CredentialStore;
pub use credentials::Credentials;
pub use prices::{PriceCache, StoredCard, StoredPrice, StoredSet};
