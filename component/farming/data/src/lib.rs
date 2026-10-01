//! The farming domain's contract, satisfied by Steam: the repository plays
//! through a client, and the client is Steam's: games to play in, a CM
//! session playing them, and what Steam says back out. Imports `farming` because the contract is declared there;
//! `farming` imports nothing back.

mod default_farming_repository;
mod farming_client;

pub use default_farming_repository::DefaultFarmingRepository;
pub use farming_client::{FarmingClient, SteamFarmingClient};
