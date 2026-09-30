//! The account's Steam inventory, as far as its community items go: trading
//! cards, emoticons, profile backgrounds, gems and booster packs, all held
//! in Steam's own app (753), context 6.
//!
//! Items are described over the CM connection by their asset IDs, with the
//! call Steam's own site makes to show an item notification:
//! `Econ.GetInventoryItemsWithDescriptions`, filtered to those IDs. What
//! each item is comes from its description's tags, read as ASF
//! (`InventoryDescription`, Apache-2.0) and Steam Economy Enhancer read them.

use std::collections::HashMap;

use anyhow::anyhow;

use crate::{
    cm::Connection,
    proto::{
        FilterOptions, GetInventoryItemsRequest, GetInventoryItemsResponse, ItemDescription, method,
    },
};

/// Steam's own app, whose inventory holds the community items.
pub(crate) const STEAM_APP: u32 = 753;
/// Where in Steam's app the community items are.
pub(crate) const COMMUNITY_CONTEXT: u64 = 6;

/// What Steam adds to a card's market name: " (Foil)" to a foil's, and the
/// others when the name clashes with another item's, as with Portal 2's
/// "Intro (Trading Card)" and "Intro (Foil Trading Card)".
const SUFFIXES: [&str; 3] = [" (Foil Trading Card)", " (Trading Card)", " (Foil)"];

/// A community item the account holds, as Steam describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItem {
    pub asset_id: u64,
    /// The game it's from: its `Game` tag (`app_620`); failing that, the app
    /// its market fee goes to; failing that, the number its market hash name
    /// starts with. `None` when none of them says.
    pub app_id: Option<u32>,
    /// Tagged `item_class_2`.
    pub trading_card: bool,
    /// Tagged `cardborder_1`.
    pub foil: bool,
    /// Its market name without any of Steam's suffixes: for a card, the name
    /// its game's set lists it by ("Intro", not "Intro (Trading Card)").
    pub name: String,
    /// Its name on the market, exactly as Steam gives it: "620-Chell (Foil)".
    pub market_hash_name: String,
    pub marketable: bool,
    pub tradable: bool,
}

/// What one ask found.
#[derive(Debug)]
pub(crate) struct Described {
    /// The items Steam described, by asset ID.
    pub(crate) items: HashMap<u64, InventoryItem>,
    /// The asset IDs Steam said it doesn't have.
    pub(crate) missing: Vec<u64>,
}

/// Asks Steam once to describe `asset_ids` among the signed-on account's
/// community items.
pub(crate) async fn describe(conn: &Connection, asset_ids: &[u64]) -> anyhow::Result<Described> {
    let steam_id = conn
        .steam_id()
        .ok_or_else(|| anyhow!("not signed on to Steam"))?;
    let answer: GetInventoryItemsResponse = conn
        .call(method::GET_INVENTORY_ITEMS, &request(steam_id, asset_ids))
        .await?;
    Ok(read(&answer))
}

/// The ask for `asset_ids`, as Steam's own site makes it.
fn request(steam_id: u64, asset_ids: &[u64]) -> GetInventoryItemsRequest {
    GetInventoryItemsRequest {
        steamid: Some(steam_id),
        appid: Some(STEAM_APP),
        contextid: Some(COMMUNITY_CONTEXT),
        get_descriptions: Some(true),
        // Names as the card pages give them, which are read in English too.
        language: Some("english".into()),
        filters: Some(FilterOptions {
            assetids: asset_ids.to_vec(),
        }),
    }
}

/// Each asset in `answer` with its description. Assets of one class and
/// instance share one; an asset whose description isn't there is left out.
fn read(answer: &GetInventoryItemsResponse) -> Described {
    let key = |class: Option<u64>, instance: Option<u64>| {
        (class.unwrap_or_default(), instance.unwrap_or_default())
    };
    let descriptions: HashMap<_, _> = answer
        .descriptions
        .iter()
        .map(|d| (key(d.classid, d.instanceid), d))
        .collect();
    let items = answer
        .assets
        .iter()
        .filter_map(|a| {
            let id = a.assetid?;
            let d = descriptions.get(&key(a.classid, a.instanceid))?;
            Some((id, item(id, d)))
        })
        .collect();
    Described {
        items,
        missing: answer
            .missing_assets
            .iter()
            .filter_map(|a| a.assetid)
            .collect(),
    }
}

fn item(asset_id: u64, d: &ItemDescription) -> InventoryItem {
    let market_name = d
        .market_name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or(d.name.as_deref())
        .unwrap_or_default();
    InventoryItem {
        asset_id,
        app_id: game(d),
        trading_card: tagged(d, "item_class", "item_class_2"),
        foil: tagged(d, "cardborder", "cardborder_1"),
        name: card_name(market_name),
        market_hash_name: d.market_hash_name.clone().unwrap_or_default(),
        marketable: d.marketable.unwrap_or_default(),
        tradable: d.tradable.unwrap_or_default(),
    }
}

/// A card's name as its game's set lists it: its market name without the
/// suffix Steam adds, if any. The market's search names cards the same way.
pub(crate) fn card_name(market_name: &str) -> String {
    SUFFIXES
        .iter()
        .find_map(|s| market_name.strip_suffix(s))
        .unwrap_or(market_name)
        .to_owned()
}

/// The game an item is from: its `Game` tag, as ASF reads it; else the app
/// its fee goes to, as Steam Game Idler reads it; else its hash name's.
fn game(d: &ItemDescription) -> Option<u32> {
    let tag = d
        .tags
        .iter()
        .filter(|t| t.category.as_deref() == Some("Game"))
        .find_map(|t| {
            t.internal_name
                .as_deref()?
                .strip_prefix("app_")?
                .parse()
                .ok()
        });
    let fee = d.market_fee_app.and_then(|app| u32::try_from(app).ok());
    // "620-Chell (Foil)": the game, a dash, the name.
    let hash = d
        .market_hash_name
        .as_deref()
        .and_then(|h| h.split_once('-'))
        .and_then(|(app, _)| app.parse().ok());
    [tag, fee, hash].into_iter().flatten().find(|&app| app != 0)
}

fn tagged(d: &ItemDescription, category: &str, internal_name: &str) -> bool {
    d.tags.iter().any(|t| {
        t.category.as_deref() == Some(category) && t.internal_name.as_deref() == Some(internal_name)
    })
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;
    use crate::proto::{Asset, ItemTag};

    fn tag(category: &str, internal_name: &str) -> ItemTag {
        ItemTag {
            category: Some(category.into()),
            internal_name: Some(internal_name.into()),
        }
    }

    /// Chell, from Portal 2, as a foil: described as Steam describes it.
    fn chell_foil() -> ItemDescription {
        ItemDescription {
            classid: Some(149_836_278),
            instanceid: Some(0),
            tradable: Some(true),
            name: Some("Chell (Foil)".into()),
            market_name: Some("Chell (Foil)".into()),
            market_hash_name: Some("620-Chell (Foil)".into()),
            marketable: Some(true),
            tags: vec![
                tag("Game", "app_620"),
                tag("item_class", "item_class_2"),
                tag("cardborder", "cardborder_1"),
            ],
            market_fee_app: Some(620),
        }
    }

    fn asset(id: u64, class: u64, instance: u64) -> Asset {
        Asset {
            assetid: Some(id),
            classid: Some(class),
            instanceid: Some(instance),
        }
    }

    #[test]
    fn asset_ids_go_unpacked_as_proto2_sends_them() {
        let filters = request(76_561_197_960_287_930, &[11, 12]).filters.unwrap();
        assert_eq!(
            filters.encode_to_vec(),
            [0x08, 11, 0x08, 12],
            "a field each"
        );
    }

    #[test]
    fn an_asset_is_described_by_its_class_and_instance() {
        let answer = GetInventoryItemsResponse {
            assets: vec![
                asset(1, 149_836_278, 0),
                asset(2, 149_836_278, 0),
                asset(3, 149_836_278, 7),
                asset(4, 5, 0),
            ],
            descriptions: vec![chell_foil()],
            missing_assets: vec![asset(9, 0, 0)],
        };

        let found = read(&answer);

        let mut ids: Vec<u64> = found.items.keys().copied().collect();
        ids.sort_unstable();
        assert_eq!(ids, [1, 2], "the others' descriptions didn't come");
        assert_eq!(found.missing, [9]);
        assert_eq!(
            found.items[&2],
            InventoryItem {
                asset_id: 2,
                app_id: Some(620),
                trading_card: true,
                foil: true,
                name: "Chell".into(),
                market_hash_name: "620-Chell (Foil)".into(),
                marketable: true,
                tradable: true,
            }
        );
    }

    #[test]
    fn the_game_is_its_tag_then_its_fee_app_then_its_hash_name() {
        let mut d = chell_foil();
        d.tags[0] = tag("Game", "app_400");
        assert_eq!(game(&d), Some(400), "the tag first");

        d.tags[0] = tag("Game", "app_");
        assert_eq!(game(&d), Some(620), "a tag without a number doesn't say");

        d.tags.remove(0);
        d.market_fee_app = Some(220);
        assert_eq!(game(&d), Some(220), "then the app the fee goes to");

        d.market_fee_app = Some(0);
        d.market_hash_name = Some("220-G-Man (Foil)".into());
        assert_eq!(game(&d), Some(220), "then the hash name, to its first dash");

        d.market_hash_name = Some("Sack of Gems".into());
        assert_eq!(game(&d), None);
    }

    #[test]
    fn a_cards_name_is_its_market_name_without_steams_suffix() {
        let name = |market_name: &str| {
            let d = ItemDescription {
                market_name: Some(market_name.into()),
                ..chell_foil()
            };
            item(1, &d).name
        };
        assert_eq!(name("Chell (Foil)"), "Chell");
        assert_eq!(name("Intro (Trading Card)"), "Intro");
        assert_eq!(name("Intro (Foil Trading Card)"), "Intro");
        assert_eq!(name("Gordon & Alyx (Foil Trading Card)"), "Gordon & Alyx");
        assert_eq!(name("G-Man"), "G-Man");
        assert_eq!(name("Portal 2 Booster Pack"), "Portal 2 Booster Pack");

        let unnamed_on_the_market = ItemDescription {
            market_name: Some(String::new()),
            name: Some("Atlas (Foil)".into()),
            ..chell_foil()
        };
        assert_eq!(item(1, &unnamed_on_the_market).name, "Atlas");
    }

    #[test]
    fn what_an_item_is_comes_from_its_tags() {
        let mut d = chell_foil();
        assert!(item(1, &d).trading_card && item(1, &d).foil);

        d.tags[2] = tag("cardborder", "cardborder_0");
        assert!(
            item(1, &d).trading_card && !item(1, &d).foil,
            "a normal card"
        );

        d.tags = vec![tag("Game", "app_620"), tag("item_class", "item_class_5")];
        assert!(!item(1, &d).trading_card, "a booster pack");
    }
}
