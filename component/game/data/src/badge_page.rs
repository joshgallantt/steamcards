//! Steam's badge pages, read as ASF reads them (`CardsFarmer.CheckPage`,
//! Apache-2.0): which games still have card drops, how many, and how long
//! each has been played. The selectors and the rules for when to trust a
//! page are ASF's, which it keeps in step with Steam's site.
//!
//! Only the account's owner sees how many drops are left, so the pages are
//! fetched signed in, as the owner (see `SteamClient::page_as_owner`).

use scraper::{ElementRef, Html};
use steam_api::page::{badge_level, decimal, digits, first, sel, text};

use crate::dto::{BadgeDto, BadgePageDto};

/// Free-to-play games whose badge row can say "no drops left" when there
/// are: ASF checks their own card page instead (`UntrustedAppIDs`).
const UNTRUSTED: [u32; 3] = [440, 570, 730];

/// Reads a page of `/badges?l=english`.
pub(crate) fn read_badge_page(html: &str) -> BadgePageDto {
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
    BadgePageDto { games, pages }
}

/// One badge row: a game with trading cards, or `None` for a badge that
/// isn't a game's (a sale's, the community's).
fn read_row(row: ElementRef<'_>) -> Option<BadgeDto> {
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
    // "Card drops received: 2". A game that's never been played can have no
    // such line, only what's still to come: nothing has dropped, and the
    // first number in its details is what's left, not what's received.
    let cards_received = stats
        .select(&sel("div[class='card_drop_info_header']"))
        .map(text)
        .find(|t| t.to_ascii_lowercase().contains("received"))
        .and_then(|t| digits(&t))
        .unwrap_or(0);
    let unsure = cards_left == 0 && cards_received == 0 && UNTRUSTED.contains(&app_id);
    Some(BadgeDto {
        app_id,
        name: name(stats, row).unwrap_or_else(|| format!("App {app_id}")),
        hours: first(stats, "div[class='badge_title_stats_playtime']")
            .map_or(0.0, |t| decimal(&text(t))),
        cards_left,
        cards_received,
        badge_level: badge_level(row),
        unsure,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn page(name: &str) -> String {
        std::fs::read_to_string(format!("{}/tests/pages/{name}", env!("CARGO_MANIFEST_DIR")))
            .unwrap()
    }

    fn game(badges: &[BadgeDto], app_id: u32) -> BadgeDto {
        badges
            .iter()
            .find(|g| g.app_id == app_id)
            .cloned()
            .unwrap_or_else(|| panic!("{app_id} isn't on the page"))
    }

    #[test]
    fn a_page_lists_each_games_cards_and_hours() {
        let page = read_badge_page(&page("badges-1.html"));

        assert_eq!(page.pages, 2);
        let ids: Vec<u32> = page.games.iter().map(|g| g.app_id).collect();
        assert_eq!(
            ids,
            [620, 440, 220, 1086940, 413150],
            "the service badge isn't a game"
        );

        let portal = game(&page.games, 620);
        assert_eq!(portal.name, "Portal 2");
        assert_eq!(portal.hours, 5.2);
        assert_eq!(portal.cards_left, 3);
        assert_eq!(portal.cards_received, 1);
        assert_eq!(portal.badge_level, 1);
        assert!(!portal.unsure);

        let unplayed = game(&page.games, 413150);
        assert_eq!((unplayed.hours, unplayed.cards_left), (0.0, 4));
        assert_eq!(unplayed.badge_level, 0, "no badge crafted yet");
    }

    #[test]
    fn a_game_never_played_has_received_nothing() {
        let page = read_badge_page(&page("badges-1.html"));
        let never = game(&page.games, 1086940);
        assert_eq!(never.name, "Baldur's Gate 3");
        assert_eq!(
            (never.hours, never.cards_left, never.cards_received),
            (0.0, 6, 0),
            "its details say only what's to come"
        );
    }

    #[test]
    fn a_finished_game_has_nothing_left() {
        let page = read_badge_page(&page("badges-1.html"));
        let done = game(&page.games, 220);
        assert_eq!(done.name, "Half-Life 2");
        assert_eq!((done.cards_left, done.cards_received), (0, 3));
        assert!(!done.unsure);
    }

    #[test]
    fn a_free_to_play_game_the_badge_may_be_wrong_about_is_checked() {
        let page = read_badge_page(&page("badges-1.html"));
        let tf2 = game(&page.games, 440);
        assert_eq!(tf2.hours, 1234.5, "commas and all");
        assert_eq!(tf2.cards_left, 0);
        assert!(
            tf2.unsure,
            "no drops and none received: ASF asks its own page"
        );
    }
}
