//! What can be asked of the cards. Each use case is a value: its type says
//! what it takes and gives, and a constructor builds the real one over the
//! repository.

use std::sync::Arc;

use tokio::task::JoinHandle;

use game::AppId;

use crate::{AssetId, CardAsset, CardError, CardRepository, CardSet, GameCards};

/// Looks at one game's cards afresh, in the background: its card page, with
/// the game's drops and hours, and its set.
pub type LookAtCards = Arc<dyn Fn(AppId) -> JoinHandle<Result<GameCards, CardError>> + Send + Sync>;

/// Looks at one game's foils afresh, in the background: each foil card of
/// its set, and how many the account has. The set a game's card page shows
/// counts normal cards only, so this is what says which copy of a foil one
/// that drops is.
pub type LookAtFoils = Arc<dyn Fn(AppId) -> JoinHandle<Result<CardSet, CardError>> + Send + Sync>;

/// Says which cards new items are, in the background, by their asset IDs:
/// each copy on its own, so a card that dropped twice comes back twice.
/// Items that aren't trading cards are left out, and so are IDs Steam
/// doesn't know.
pub type DescribeCards =
    Arc<dyn Fn(Vec<AssetId>) -> JoinHandle<Result<Vec<CardAsset>, CardError>> + Send + Sync>;

pub fn look_at_cards(repo: Arc<dyn CardRepository>) -> LookAtCards {
    Arc::new(move |app_id| {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            repo.game_cards(app_id)
                .await
                .map_err(|e| CardError::Unavailable(e.to_string()))
        })
    })
}

pub fn look_at_foils(repo: Arc<dyn CardRepository>) -> LookAtFoils {
    Arc::new(move |app_id| {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            repo.foils(app_id)
                .await
                .map_err(|e| CardError::Unavailable(e.to_string()))
        })
    })
}

pub fn describe_cards(repo: Arc<dyn CardRepository>) -> DescribeCards {
    Arc::new(move |asset_ids| {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            repo.describe(&asset_ids)
                .await
                .map_err(|e| CardError::Unavailable(e.to_string()))
        })
    })
}
