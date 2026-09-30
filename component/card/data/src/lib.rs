//! The card domain's contract, satisfied by Steam: the repository reads the
//! cards through a client, and the client is Steam's, reading card pages
//! (foils' too) from steamcommunity.com and the inventory's items over the
//! CM connection. Imports `card` because the contract is declared there;
//! `card` imports nothing back.

mod card_client;
mod default_card_repository;

pub use card_client::{CardClient, SteamCardClient};
pub use default_card_repository::DefaultCardRepository;
