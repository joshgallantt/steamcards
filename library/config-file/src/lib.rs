//! The JSON files steamcards keeps: the config file, where each part of
//! steamcards keeps its own fields beside the others', and files of their
//! own beside it, like the market's prices (see [`JsonFile`]). Where they
//! live is the composition root's decision; this crate opens the paths it is
//! given.
//!
//! Storage only: it knows how a file is kept, not what's in it. Each data
//! crate declares the shape of what it keeps; the saved sign-in's is here,
//! since the Steam client, a library, keeps it.
//!
//! A file can hold a sign-in, so on macOS and Linux only its owner can read
//! it.

mod config_file;
mod credential_store;
mod credentials;
mod json_file;
mod private_file;

pub use config_file::ConfigFile;
pub use credential_store::CredentialStore;
pub use credentials::Credentials;
pub use json_file::JsonFile;
