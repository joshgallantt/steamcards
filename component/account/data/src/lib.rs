//! The account domain's contract, satisfied by Steam: the repository keeps
//! the sign-in through a client, and the client is Steam's, signing in with
//! a QR code the Steam app scans and approves. Imports `account` because the
//! contract is declared there; `account` imports nothing back.

mod account_client;
mod default_account_repository;

pub use account_client::{AccountClient, SteamAccountClient};
pub use default_account_repository::DefaultAccountRepository;
