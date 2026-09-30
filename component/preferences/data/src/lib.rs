//! The preferences domain's contract, satisfied by the config file: the
//! repository keeps the preferences through a store, and the store keeps
//! them in the file, in a shape of their own. Imports `preferences` because
//! the contract is declared there; `preferences` imports nothing back.

mod default_preferences_repository;
mod dto;
mod preferences_store;

pub use default_preferences_repository::DefaultPreferencesRepository;
pub use preferences_store::{FilePreferencesStore, PreferencesStore};
