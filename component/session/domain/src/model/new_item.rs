use card::AssetId;
use chrono::{DateTime, Utc};
use steam_library::AppId;

/// An item Steam announced as new in the account's inventory: perhaps a
/// card that just dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewItem {
    /// Its ID in the inventory, which says which card it is.
    pub asset_id: AssetId,
    /// The game it came from, when Steam says.
    pub app_id: Option<AppId>,
    /// When it arrived, when Steam says.
    pub gained_at: Option<DateTime<Utc>>,
}
