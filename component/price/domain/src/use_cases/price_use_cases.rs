//! Everything asked of the market, a trait each. Each is done over the
//! repository by a `Default…UseCase` in `impl/`. A view model holds only the
//! ones it calls, and a test double is a type of its own in `test_support`.
//!
//! The market decides what to price, and in what order. How fast requests
//! go is Steam's business: every lookup waits its turn in the data layer's
//! one market queue. An answer that can't be used is tried again a day
//! later; a market that couldn't be asked at all, soon, with nothing taken
//! as failed.

use std::sync::Arc;

use game::AppId;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{PriceBook, PriceError, PriceEvent, Wallet};

/// Everything priced so far.
pub trait GetPricesUseCase: Send + Sync {
    fn call(&self) -> Arc<PriceBook>;
}

/// Says which games to price, most urgent first: the game being farmed,
/// then games with cards this session, then the rest in farm order.
pub trait SetGamesToPriceUseCase: Send + Sync {
    fn call(&self, app_ids: Vec<AppId>);
}

/// Prices the games to price, normal cards and foils, until the token is
/// cancelled, reporting on the channel: each set once, then again once its
/// prices are 6 hours old, in the order asked for. While Steam has paused
/// lookups, it waits. Runs detached; the handle resolves once it has
/// stopped.
pub trait KeepPricesUpToDateUseCase: Send + Sync {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<PriceEvent>) -> JoinHandle<()>;
}

/// Prices a game's set again in the background, if its prices are over an
/// hour old: one of its cards just dropped, or the user asked. Errs when
/// Steam has paused lookups, or the market couldn't be asked.
pub trait RefreshPricesUseCase: Send + Sync {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<(), PriceError>>;
}

/// The account's wallet: its currency, and the fees Steam takes. `None`
/// until Steam has said.
pub trait GetWalletUseCase: Send + Sync {
    fn call(&self) -> Option<Wallet>;
}
