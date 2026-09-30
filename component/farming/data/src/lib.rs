//! The farming domain's contract, satisfied by Steam: games to play in, a CM
//! session playing them, and what Steam says back out. Imports `farming`
//! because the contract is declared there; `farming` imports nothing back.

mod play;

pub use play::SteamFarmingRepository;
