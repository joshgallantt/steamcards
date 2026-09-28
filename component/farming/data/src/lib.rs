//! The farming domain's contracts, satisfied by Steam: badge pages in, games
//! out; games to play in, a CM session playing them. Imports `farming`
//! because the contracts are declared there; `farming` imports nothing back.

mod cards;
mod play;

pub use cards::SteamCardsRepository;
pub use play::SteamPlayRepository;
