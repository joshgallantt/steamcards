use game::AppId;

use crate::{AssetId, CardAsset};

/// A copy of `name` from `app_id`'s set, held as `asset_id`: not a foil,
/// marketable and tradable, with the market hash name Steam would give it.
pub fn card_asset(asset_id: u64, app_id: u32, name: &str) -> CardAsset {
    CardAsset {
        asset_id: AssetId(asset_id),
        app_id: AppId(app_id),
        name: name.to_owned(),
        market_hash_name: format!("{app_id}-{name}"),
        foil: false,
        marketable: true,
        tradable: true,
    }
}
