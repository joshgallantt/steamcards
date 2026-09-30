use anyhow::{anyhow, bail};
use serde::Deserialize;
use steam_api::inventory::{STEAM_APP, card_name};

/// The most pages of a set read: 10 cards a page, and a set has 15 at most.
pub(crate) const MAX_SET_PAGES: u32 = 10;

/// A card of a game's set as the market lists it: one result of
/// `search/render`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Listed {
    /// Its name on the market, exactly as Steam gives it:
    /// "620-Intro (Trading Card)".
    pub(crate) hash_name: String,
    /// Its name as its game's set lists it: the market's name for it
    /// ("Intro (Trading Card)") without Steam's suffix.
    pub(crate) name: String,
    /// Its lowest listing, what a buyer pays, in hundredths: `sell_price`.
    pub(crate) sell_price: i64,
    /// The same, as the market writes it: the only sign of its currency.
    pub(crate) sell_price_text: String,
    /// How many are listed.
    pub(crate) sell_listings: u32,
    /// What it is: "Portal 2 Trading Card", or "Portal 2 Foil Trading
    /// Card".
    pub(crate) item_type: String,
}

/// `search/render` for a page of a game's cards: normal ones, or foils, 10
/// to a page from `start`, by name. Exactly as research §1.1 has it.
pub(crate) fn search_path(app_id: u32, foil: bool, start: u32) -> String {
    format!(
        "/market/search/render/?norender=1&appid={STEAM_APP}\
         &category_{STEAM_APP}_Game%5B%5D=tag_app_{app_id}\
         &category_{STEAM_APP}_item_class%5B%5D=tag_item_class_2\
         &category_{STEAM_APP}_cardborder%5B%5D=tag_cardborder_{}\
         &sort_column=name&sort_dir=asc&start={start}",
        u8::from(foil)
    )
}

/// One page of a game's cards.
#[derive(Debug)]
pub(crate) struct SearchPage {
    pub(crate) listed: Vec<Listed>,
    /// How many cards there are in all.
    pub(crate) total: u32,
    /// How many a page holds: 10, whatever's asked.
    pub(crate) page_size: u32,
}

#[derive(Deserialize)]
struct SearchReply {
    success: bool,
    #[serde(default)]
    pagesize: u32,
    #[serde(default)]
    total_count: u32,
    #[serde(default)]
    results: Vec<SearchResult>,
}

// `sale_price_text` is left unread: it's undocumented, and not what a
// seller gets (research §1.1).
#[derive(Deserialize)]
struct SearchResult {
    hash_name: String,
    name: String,
    #[serde(default)]
    sell_price: i64,
    #[serde(default)]
    sell_price_text: String,
    #[serde(default)]
    sell_listings: u32,
    #[serde(default)]
    asset_description: AssetDescription,
}

#[derive(Deserialize, Default)]
struct AssetDescription {
    #[serde(default, rename = "type")]
    item_type: String,
}

/// Reads a page of `search/render`.
pub(crate) fn read_search(body: &str) -> anyhow::Result<SearchPage> {
    let reply: SearchReply = serde_json::from_str(body)
        .map_err(|e| anyhow!("the market's list of cards didn't read ({e})"))?;
    if !reply.success {
        bail!("the market didn't list the cards");
    }
    Ok(SearchPage {
        total: reply.total_count,
        page_size: reply.pagesize,
        listed: reply
            .results
            .into_iter()
            .map(|r| Listed {
                name: card_name(&r.name),
                hash_name: r.hash_name,
                sell_price: r.sell_price,
                sell_price_text: r.sell_price_text,
                sell_listings: r.sell_listings,
                item_type: r.asset_description.item_type,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sets_cards_are_asked_for_as_steams_own_pages_ask() {
        assert_eq!(
            search_path(620, false, 0),
            "/market/search/render/?norender=1&appid=753\
             &category_753_Game%5B%5D=tag_app_620\
             &category_753_item_class%5B%5D=tag_item_class_2\
             &category_753_cardborder%5B%5D=tag_cardborder_0\
             &sort_column=name&sort_dir=asc&start=0"
        );
        assert!(
            search_path(220, true, 10)
                .ends_with("tag_cardborder_1&sort_column=name&sort_dir=asc&start=10")
        );
    }

    #[test]
    fn a_page_of_cards_reads_as_the_market_wrote_it() {
        // From a live answer for Portal 2's cards, 2026-09-29, cut short.
        let body = r#"{"success":true,"start":0,"pagesize":10,"total_count":16,
            "searchdata":{"query":"","total_count":16,"pagesize":10},
            "results":[
              {"name":"Chell","hash_name":"620-Chell","sell_listings":4662,"sell_price":6,
               "sell_price_text":"$0.06","app_name":"Steam",
               "asset_description":{"appid":753,"classid":"149749395","type":"Portal 2 Trading Card",
                 "market_name":"Chell","market_hash_name":"620-Chell","commodity":1},
               "sale_price_text":"$0.05"},
              {"name":"Intro (Foil Trading Card)","hash_name":"620-Intro (Foil Trading Card)",
               "sell_listings":97,"sell_price":63,"sell_price_text":"$0.63",
               "asset_description":{"type":"Portal 2 Foil Trading Card"},
               "sale_price_text":"$0.61"}]}"#;

        let page = read_search(body).unwrap();

        assert_eq!((page.total, page.page_size), (16, 10));
        assert_eq!(
            page.listed,
            [
                Listed {
                    hash_name: "620-Chell".into(),
                    name: "Chell".into(),
                    sell_price: 6,
                    sell_price_text: "$0.06".into(),
                    sell_listings: 4662,
                    item_type: "Portal 2 Trading Card".into(),
                },
                Listed {
                    hash_name: "620-Intro (Foil Trading Card)".into(),
                    name: "Intro".into(),
                    sell_price: 63,
                    sell_price_text: "$0.63".into(),
                    sell_listings: 97,
                    item_type: "Portal 2 Foil Trading Card".into(),
                },
            ]
        );
        assert!(read_search(r#"{"success":false}"#).is_err());
        assert!(read_search("<html></html>").is_err());
    }
}
