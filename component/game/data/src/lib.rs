//! The game domain's contract, satisfied by Steam: the repository reads the
//! library through a client, and the client is Steam's, reading the badge
//! pages on steamcommunity.com, and a game's own card page when they may be
//! wrong about it. Imports `game` because the contract is declared there;
//! `game` imports nothing back.

mod badge_page;
mod default_game_repository;
mod dto;
mod game_client;

pub use default_game_repository::DefaultGameRepository;
pub use game_client::{GameClient, SteamGameClient};
