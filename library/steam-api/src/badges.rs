//! A game's own card page on steamcommunity.com, and its foil badge's, read
//! as ASF reads them (`GetGameCardsInfo`, Apache-2.0): the game's drops and
//! hours, and its set of cards. The selectors and the rules for when to
//! trust a page are ASF's, which it keeps in step with Steam's site.
//!
//! Both the game and card data crates read this page, so it's read here,
//! once. Only the account's owner sees how many drops are left, so it's
//! fetched signed in (see `community`); a page shown signed out says so.

use scraper::{ElementRef, Html};

use crate::{
    inventory::card_name,
    page::{badge_level, decimal, digits, first, sel, text, viewer},
};

/// A game's cards, as its badge shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct BadgeGame {
    pub app_id: u32,
    pub name: String,
    /// Hours on record.
    pub hours: f64,
    /// Card drops still to come from playing it.
    pub cards_left: u32,
    /// Card drops so far.
    pub cards_received: u32,
    /// The badge's level: 0 until one is crafted.
    pub badge_level: u8,
    /// Its set of cards, and how many of each the account has.
    pub cards: Vec<SetCard>,
}

/// A card in a game's set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCard {
    pub name: String,
    /// How many the account has.
    pub owned: u32,
}

/// A game's own card page.
#[derive(Debug, Clone, PartialEq)]
pub struct GameCardsPage {
    /// `None` when the page has no card drops on it to read.
    pub game: Option<BadgeGame>,
    pub viewer: Option<u64>,
}

/// Reads `/gamecards/<app id>?l=english`.
pub fn read_game_cards_page(app_id: u32, html: &str) -> GameCardsPage {
    let doc = Html::parse_document(html);
    let root = doc.root_element();
    let game = first(root, "span[class='progress_info_bold']").map(|progress| {
        let name = doc
            .select(&sel("span[class='profile_small_header_location']"))
            .next_back()
            .map(text)
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| format!("App {app_id}"));
        BadgeGame {
            app_id,
            name,
            hours: first(root, "div[class='badge_title_stats_playtime']")
                .map_or(0.0, |t| decimal(&text(t))),
            cards_left: digits(&text(progress)).unwrap_or(0),
            cards_received: first(root, "div[class='card_drop_info_header']")
                .and_then(|h| digits(&text(h)))
                .unwrap_or(0),
            badge_level: badge_level(root),
            cards: set(root),
        }
    });
    GameCardsPage {
        game,
        viewer: viewer(html),
    }
}

/// Reads `/gamecards/<app id>?border=1&l=english`: the card page of a
/// game's foil badge, whose set is its foils, and how many of each the
/// account has. It's laid out as the normal page is. Each foil is named as
/// the set names the card, without any suffix the market would add.
pub fn read_foil_cards_page(html: &str) -> Vec<SetCard> {
    let doc = Html::parse_document(html);
    set(doc.root_element())
        .into_iter()
        .map(|card| SetCard {
            name: card_name(&card.name),
            ..card
        })
        .collect()
}

/// The cards in the set, as ASF finds them (`GetCardCountForGame`): each a
/// child of the set whose class starts `badge_card_set_card`, marked
/// `unowned` when the account has none. A card's name follows its quantity:
/// `<div class="badge_card_set_text_qty">(2)</div>Atlas`.
fn set(scope: ElementRef<'_>) -> Vec<SetCard> {
    scope
        .select(&sel(
            "div[class='badge_card_set_cards'] > div[class^='badge_card_set_card']",
        ))
        .filter_map(|card| {
            let label = first(card, "div[class^='badge_card_set_text']")?;
            let qty = first(label, "div[class='badge_card_set_text_qty']").map(text);
            let name = text(label);
            let name = qty
                .as_deref()
                .and_then(|q| name.strip_prefix(q))
                .unwrap_or(&name)
                .trim()
                .to_owned();
            let unowned = card.value().classes().any(|c| c == "unowned");
            let owned = if unowned {
                0
            } else {
                qty.as_deref().and_then(digits).unwrap_or(1)
            };
            (!name.is_empty()).then_some(SetCard { name, owned })
        })
        .collect()
}
