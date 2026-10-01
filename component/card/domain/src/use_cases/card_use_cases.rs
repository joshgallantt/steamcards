//! Everything asked of the cards, a trait each: looking at them, telling
//! which card an item is, and what they're worth on the market. Each is
//! done over a repository by a `Default…UseCase` in `impl/`.
//!
//! Pricing decides what to price, and in what order. How fast requests go
//! is Steam's business: every lookup waits its turn in the data layer's one
//! market queue. An answer that can't be used is tried again a day later; a
//! market that couldn't be asked at all, soon, with nothing taken as
//! failed.

use std::sync::Arc;

use async_trait::async_trait;
use game::AppId;
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::{
    AssetId, CardAsset, CardError, CardSet, GameCards, NewItem, PriceBook, PriceError, PriceEvent,
};

/// Looks at one game's cards afresh, in the background: its card page, with
/// the game's drops and hours, and its set.
pub trait LookAtCardsUseCase: Send + Sync {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<GameCards, CardError>>;
}

/// Looks at one game's set in foil afresh, in the background: each card of
/// it, and how many foils of it the account has. The set a game's card page
/// shows is the normal one, so this is what says which copy of a foil one
/// that drops is. A page of its own: farming reads it only when a foil
/// drops, rather than ask Steam twice at every look.
pub trait LookAtFoilsUseCase: Send + Sync {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<CardSet, CardError>>;
}

/// Says which cards new items are, in the background, by their asset IDs:
/// each copy on its own, so a card that dropped twice comes back twice.
/// Items that aren't trading cards are left out, and so are IDs Steam
/// doesn't know.
pub trait IdentifyCardsUseCase: Send + Sync {
    fn call(&self, asset_ids: Vec<AssetId>) -> JoinHandle<Result<Vec<CardAsset>, CardError>>;
}

/// What Steam says next is new in the account's inventory, once it says
/// it: perhaps a card that just dropped. The items it lists that weren't
/// heard of before, each once; none when it only counts more. Waited on in
/// place: dropped before Steam has said, as a `select!` drops the branches
/// that lose, nothing is missed.
#[async_trait]
pub trait ObserveNewItemsUseCase: Send + Sync {
    async fn call(&self) -> Vec<NewItem>;
}

/// Every card priced so far.
pub trait GetCardPricesUseCase: Send + Sync {
    fn call(&self) -> Arc<PriceBook>;
}

/// Says which games' cards to price, most urgent first: the game being
/// farmed, then games with cards this session, then the rest in farm order.
pub trait SetCardsToPriceUseCase: Send + Sync {
    fn call(&self, app_ids: Vec<AppId>);
}

/// Prices the cards of the games to price, normal and foil, until the token
/// is cancelled, reporting on the channel: each set once, then again once
/// its prices are 6 hours old, in the order asked for. While Steam has
/// paused lookups, it waits. Runs detached; the handle resolves once it has
/// stopped.
pub trait KeepCardPricesUpToDateUseCase: Send + Sync {
    fn call(&self, token: CancellationToken, events: mpsc::Sender<PriceEvent>) -> JoinHandle<()>;
}

/// Prices a game's set again in the background, if its prices are over an
/// hour old: one of its cards just dropped, or the user asked. Errs when
/// Steam has paused lookups, or the market couldn't be asked.
pub trait RefreshCardPricesUseCase: Send + Sync {
    fn call(&self, app_id: AppId) -> JoinHandle<Result<(), PriceError>>;
}
