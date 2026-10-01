//! Everything asked of the cards, a trait each. Each is done over the
//! repository by a `Default…UseCase` in `impl/`.

use steam_library::AppId;
use tokio::task::JoinHandle;

use crate::{AssetId, CardAsset, CardError, CardSet, GameCards};

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
