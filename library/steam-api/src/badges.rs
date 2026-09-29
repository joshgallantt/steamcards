//! Steam's badge pages, read as ASF reads them (`CardsFarmer.CheckPage` and
//! `GetGameCardsInfo`, Apache-2.0): which games still have card drops, how
//! many, and how long each has been played. The selectors and the rules for
//! when to trust a page are ASF's, which it keeps in step with Steam's site.
//!
//! Only the account's owner sees how many drops are left, so the pages are
//! fetched signed in (see `community`); a page shown signed out says so.

use scraper::{ElementRef, Html, Selector};

/// Free-to-play games whose badge row can say "no drops left" when there
/// are: ASF checks their own card page instead (`UntrustedAppIDs`).
const UNTRUSTED: [u32; 3] = [440, 570, 730];

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
    /// The badge page may be wrong about it: ask the game's own card page.
    pub unsure: bool,
    /// Its set of cards, and how many of each the account has: only a game's
    /// own card page shows them, so empty from the badge pages.
    pub cards: Vec<SetCard>,
}

/// A card in a game's set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetCard {
    pub name: String,
    /// How many the account has.
    pub owned: u32,
}

/// One page of badges.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BadgePage {
    /// The games with trading cards on this page.
    pub games: Vec<BadgeGame>,
    /// How many pages of badges there are.
    pub pages: u32,
    /// Whose eyes the page was shown to; `None` when signed out.
    pub viewer: Option<u64>,
}

/// A game's own card page.
#[derive(Debug, Clone, PartialEq)]
pub struct GameCardsPage {
    /// `None` when the page has no card drops on it to read.
    pub game: Option<BadgeGame>,
    pub viewer: Option<u64>,
}

/// Reads a page of `/badges?l=english`.
pub fn read_badge_page(html: &str) -> BadgePage {
    let doc = Html::parse_document(html);
    let pages = doc
        .select(&sel("a[class='pagelink']"))
        .next_back()
        .and_then(|a| text(a).parse().ok())
        .unwrap_or(1);
    let games = doc
        .select(&sel("div[class='badge_row_inner']"))
        .filter_map(read_row)
        .collect();
    BadgePage {
        games,
        pages,
        viewer: viewer(html),
    }
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
            badge_level: level(root),
            unsure: false,
            cards: set(root),
        }
    });
    GameCardsPage {
        game,
        viewer: viewer(html),
    }
}

/// One badge row: a game with trading cards, or `None` for a badge that
/// isn't a game's (a sale's, the community's).
fn read_row(row: ElementRef<'_>) -> Option<BadgeGame> {
    let stats = first(row, "div[class='badge_title_stats_content']")?;
    let dialog = first(stats, "div[class='card_drop_info_dialog'][id]")?;
    // "card_drop_info_gamebadge_730_1_0": the app ID is the fifth part.
    let app_id: u32 = dialog.value().attr("id")?.split('_').nth(4)?.parse().ok()?;
    if app_id == 0 {
        return None;
    }
    // "3 card drops remaining", or "No card drops remaining" without a number.
    let cards_left = first(stats, "span[class='progress_info_bold']")
        .and_then(|p| digits(&text(p)))
        .unwrap_or(0);
    let cards_received = first(stats, "div[class='card_drop_info_header']")
        .and_then(|h| digits(&text(h)))
        .unwrap_or(0);
    let unsure = cards_left == 0 && cards_received == 0 && UNTRUSTED.contains(&app_id);
    Some(BadgeGame {
        app_id,
        name: name(stats, row).unwrap_or_else(|| format!("App {app_id}")),
        hours: first(stats, "div[class='badge_title_stats_playtime']")
            .map_or(0.0, |t| decimal(&text(t))),
        cards_left,
        cards_received,
        badge_level: level(row),
        unsure,
        cards: Vec::new(),
    })
}

/// The game's name, from the last line of its card drop details ("You can
/// get 3 more trading cards by playing Portal 2."), else the badge's title.
fn name(stats: ElementRef<'_>, row: ElementRef<'_>) -> Option<String> {
    let body = stats
        .select(&sel("div[class='card_drop_info_body']"))
        .last();
    let from_body = body.map(text).and_then(|t| {
        let after = ["by playing ", "drops remaining for "]
            .iter()
            .find_map(|lead| t.find(lead).map(|i| i + lead.len()))?;
        let end = t.rfind('.').filter(|&end| end > after)?;
        Some(t[after..end].trim().to_owned())
    });
    from_body.filter(|n| !n.is_empty()).or_else(|| {
        let title = text(first(row, "div[class='badge_title']")?);
        let title = title.trim_end_matches("View details").trim().to_owned();
        (!title.is_empty()).then_some(title)
    })
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

/// "Level 3, 300 XP" under the badge: 0 until one is crafted.
fn level(scope: ElementRef<'_>) -> u8 {
    scope
        .select(&sel("div[class='badge_info_description'] > div"))
        .nth(1)
        .map(text)
        .and_then(|t| {
            let rest = &t[t.find("Level ")? + "Level ".len()..];
            rest.split(',').next()?.trim().parse().ok()
        })
        .filter(|&l| l <= 5)
        .unwrap_or(0)
}

/// The account the page was shown to, from the site's own script:
/// `g_steamID = "7656…";`, or `g_steamID = false;` when signed out (xPaw's
/// farmer checks for the same).
pub(crate) fn viewer(html: &str) -> Option<u64> {
    match seen_by(html) {
        Seen::By(id) => Some(id),
        Seen::SignedOut | Seen::Unknown => None,
    }
}

/// Who a page says it was shown to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seen {
    By(u64),
    SignedOut,
    /// The page doesn't say: not one of the site's usual pages.
    Unknown,
}

pub(crate) fn seen_by(html: &str) -> Seen {
    let Some(at) = html.find("g_steamID") else {
        return Seen::Unknown;
    };
    let Some(rest) = html[at + "g_steamID".len()..]
        .trim_start()
        .strip_prefix('=')
    else {
        return Seen::Unknown;
    };
    let rest = rest.trim_start();
    if rest.starts_with("false") {
        return Seen::SignedOut;
    }
    let Some(quote) = rest.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return Seen::Unknown;
    };
    let rest = &rest[1..];
    rest.find(quote)
        .and_then(|end| rest[..end].parse().ok())
        .filter(|&id| id != 0)
        .map_or(Seen::Unknown, Seen::By)
}

fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("the selectors here are written by hand, and valid")
}

fn first<'a>(scope: ElementRef<'a>, css: &str) -> Option<ElementRef<'a>> {
    scope.select(&sel(css)).next()
}

/// An element's text, without the spaces around it (non-breaking ones too).
fn text(e: ElementRef<'_>) -> String {
    e.text().collect::<String>().trim().to_owned()
}

/// The first whole number in `s`.
fn digits(s: &str) -> Option<u32> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let run: String = s[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    run.parse().ok()
}

/// The first number in `s`, commas and all: "1,234.5 hrs on record".
fn decimal(s: &str) -> f64 {
    let Some(start) = s.find(|c: char| c.is_ascii_digit()) else {
        return 0.0;
    };
    let run: String = s[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
        .filter(|c| *c != ',')
        .collect();
    run.trim_end_matches('.').parse().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_read_as_steam_writes_them() {
        assert_eq!(digits("3 card drops remaining"), Some(3));
        assert_eq!(digits("No card drops remaining"), None);
        assert_eq!(digits("Card drops received: 12"), Some(12));
        assert_eq!(decimal("\u{a0}5.2 hrs on record"), 5.2);
        assert_eq!(decimal("1,234.5 hrs on record"), 1234.5);
        assert_eq!(decimal(""), 0.0);
    }

    #[test]
    fn the_viewer_is_whoever_the_page_says() {
        assert_eq!(
            viewer(r#"<script>g_steamID = "76561197960287930";</script>"#),
            Some(76_561_197_960_287_930)
        );
        assert_eq!(
            seen_by("<script>g_steamID='76561197960287930';</script>"),
            Seen::By(76_561_197_960_287_930)
        );
        assert_eq!(
            seen_by("<script>g_steamID = false;</script>"),
            Seen::SignedOut
        );
        assert_eq!(seen_by("<html></html>"), Seen::Unknown);
        assert_eq!(viewer("<script>g_steamID = false;</script>"), None);
    }
}
