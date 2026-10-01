use card::{CardAsset, CardKind};
use steam_library::AppId;

/// A card the account holds, as far as its value goes: a copy that dropped,
/// or one known only by its name and border.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldCard {
    /// The game whose set it's from.
    pub app_id: AppId,
    /// Its name as the game's set lists it.
    pub name: String,
    pub kind: CardKind,
    /// Its exact name on the market, when its copy was described.
    pub market_hash_name: Option<String>,
    pub marketable: bool,
}

impl HeldCard {
    /// A card known only by its name and kind: from its game's card page,
    /// when its copy couldn't be described. Its price is its set's, by name.
    pub fn named(app_id: AppId, name: &str, kind: CardKind) -> Self {
        Self {
            app_id,
            name: name.to_owned(),
            kind,
            market_hash_name: None,
            marketable: true,
        }
    }
}

impl From<&CardAsset> for HeldCard {
    fn from(asset: &CardAsset) -> Self {
        Self {
            app_id: asset.app_id,
            name: asset.name.clone(),
            kind: asset.kind,
            market_hash_name: Some(asset.market_hash_name.clone()),
            marketable: asset.marketable,
        }
    }
}

#[cfg(test)]
mod tests {
    use card::AssetId;
    use steam_library::AppId;

    use super::*;

    #[test]
    fn a_dropped_copy_is_held_as_the_card_it_is() {
        let asset = CardAsset {
            asset_id: AssetId(31_002),
            app_id: AppId(960_910),
            name: "Madison".into(),
            market_hash_name: "960910-Madison".into(),
            kind: CardKind::Normal,
            marketable: true,
            tradable: true,
        };
        assert_eq!(
            HeldCard::from(&asset),
            HeldCard {
                app_id: AppId(960_910),
                name: "Madison".into(),
                kind: CardKind::Normal,
                market_hash_name: Some("960910-Madison".into()),
                marketable: true,
            }
        );
    }
}
