//! How steamcommunity.com's pages are read, whichever page it is: numbers as
//! Steam writes them, the badge under a game, and who a page was shown to.
//! The data crates that read a page of their own read it with these, so a
//! change to the site's ways is made here, once.

use scraper::{ElementRef, Selector};

/// "Level 3, 300 XP" under the badge: 0 until one is crafted.
pub fn badge_level(scope: ElementRef<'_>) -> u8 {
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
pub fn viewer(html: &str) -> Option<u64> {
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

/// A selector written by hand, so known to be valid.
pub fn sel(css: &str) -> Selector {
    Selector::parse(css).expect("the selectors here are written by hand, and valid")
}

/// The first element in `scope` that `css` selects.
pub fn first<'a>(scope: ElementRef<'a>, css: &str) -> Option<ElementRef<'a>> {
    scope.select(&sel(css)).next()
}

/// An element's text, without the spaces around it (non-breaking ones too).
pub fn text(e: ElementRef<'_>) -> String {
    e.text().collect::<String>().trim().to_owned()
}

/// The first whole number in `s`.
pub fn digits(s: &str) -> Option<u32> {
    let start = s.find(|c: char| c.is_ascii_digit())?;
    let run: String = s[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    run.parse().ok()
}

/// The first number in `s`, commas and all: "1,234.5 hrs on record".
pub fn decimal(s: &str) -> f64 {
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
