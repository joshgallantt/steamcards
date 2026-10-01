//! Everything asked of the games, a trait each: reading the library, and
//! playing. Each is done over a repository by a `Default…UseCase` in
//! `impl/`.
//!
//! Playing is asked in place, and waited on there: a future dropped before
//! it's done, as a `select!` drops the branches that lose, takes nothing
//! with it. A signal not heard yet is still there for the next ask.

use async_trait::async_trait;
use tokio::task::JoinHandle;

use crate::{AppId, GameError, Playing, PlayingSignal, SteamLibrary};

/// Reads the library in the background: games with drops left first, most
/// played first, then finished ones, by name.
pub trait GetLibraryUseCase: Send + Sync {
    fn call(&self) -> JoinHandle<Result<SteamLibrary, GameError>>;
}

/// Plays exactly these games, signing on first if need be, and shows to
/// friends as online or not. Says who plays: this session, or another
/// device, which keeps it from playing anything, as Steam would sign it off.
#[async_trait]
pub trait PlayGamesUseCase: Send + Sync {
    async fn call(&self, app_ids: &[AppId], online: bool) -> Result<Playing, GameError>;
}

/// Stands by to play: signs on if need be, playing nothing, so Steam can
/// say when another device stops playing. Says who plays now.
#[async_trait]
pub trait StandByUseCase: Send + Sync {
    async fn call(&self) -> Result<Playing, GameError>;
}

/// Stops playing, and signs off.
#[async_trait]
pub trait StopPlayingUseCase: Send + Sync {
    async fn call(&self);
}

/// What Steam says next about playing on the account, once it says it.
#[async_trait]
pub trait ObservePlayingUseCase: Send + Sync {
    async fn call(&self) -> PlayingSignal;
}
