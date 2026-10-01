//! The game domain's contracts, satisfied by Steam. One repository reads the
//! library through a client, Steam's, reading the badge pages on
//! steamcommunity.com, and a game's own card page when they may be wrong
//! about it. The other plays through a client of its own, telling Steam
//! what's played over the CM connection and hearing what it says back.
//! Imports `game` because the contracts are declared there; `game` imports
//! nothing back.

mod badge_page;
mod default_game_repository;
mod default_playing_repository;
mod dto;
mod game_client;
mod playing_client;

pub use default_game_repository::DefaultGameRepository;
pub use default_playing_repository::DefaultPlayingRepository;
pub use game_client::{GameClient, SteamGameClient};
pub use playing_client::{PlayingClient, SteamPlayingClient};
