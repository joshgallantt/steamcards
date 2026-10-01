//! The steam-library domain's contract, satisfied by Steam: the repository
//! reads the library through a client, and the client is Steam's, reading
//! the badge pages on steamcommunity.com, and a game's own card page when
//! they may be wrong about it. Imports `steam-library` because the contract
//! is declared there; `steam-library` imports nothing back.

mod badge_page;
mod default_steam_library_repository;
mod dto;
mod library_client;

pub use default_steam_library_repository::DefaultSteamLibraryRepository;
pub use library_client::{LibraryClient, SteamLibraryClient};
