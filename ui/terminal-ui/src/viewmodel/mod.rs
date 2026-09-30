// Presentation-layer view models. Depend only on domain use cases and entities —
// never on repositories, data sources, or any data-layer type.

mod account;
mod chosen;
mod farming;
#[cfg(test)]
pub(crate) mod fixtures;
mod games;
mod haul;
mod library;
mod log;
mod login;
mod market;
mod now;
mod onboarding;
mod progress;
mod queue;
mod screen;

pub use account::{Account, AccountView};
pub use chosen::{ChosenGame, Doing, SessionCard, SetCard, TheSet};
pub use farming::Farming;
pub use games::{GameRow, Games};
pub use haul::{Best, GameHaul, GameState, Haul, HaulRow, Pace, Track, Unpriced, Why};
pub use library::Library;
pub use log::{Alert, Detail, Flash, LogEntry, LogKind, Strip, market_line};
pub use login::{Login, LoginUpdate};
pub use market::{Banner, Market, MarketNews, MarketRow, MarketView, Priced};
pub use now::{GameNow, Group, GroupGame, LastDrop, NextCard, Now, Pips, Told};
pub use onboarding::{Job, NeedsAccount, Onboarding, Step};
pub use progress::{
    Checked, Eta, Learnt, LibraryProgress, Progress, SessionProgress, Summary, ToGo, learnt,
};
pub use queue::{
    Glyph, MOST_PIPS, Queue, QueueEntry, RowStatus, Section, SectionDoing, SectionRows,
};
pub use screen::{
    Activity, Cell, Header, Pricing, Run, Snapshot, Values, held_card, session_value,
};

/// Opens a page in the default browser: through AppKit on macOS, and through
/// `$BROWSER` or the desktop's opener (`xdg-open` and the like) on Linux. A
/// graphical browser's output never reaches the TUI; a text one, like lynx,
/// has the terminal until it quits.
pub fn open_in_browser(url: &str) -> std::io::Result<()> {
    webbrowser::open(url)
}

/// A game's card page on steamcommunity.com, for the signed-in account.
pub fn card_page(app_id: u32) -> String {
    format!("https://steamcommunity.com/my/gamecards/{app_id}/")
}

/// A card's page on the Steam market, by its market hash name: its listings
/// and its offers. Trading cards are Steam's own items, app 753.
pub fn market_page(market_hash_name: &str) -> String {
    let mut path = String::new();
    for byte in market_hash_name.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                path.push(char::from(byte));
            }
            b => path.push_str(&format!("%{b:02X}")),
        }
    }
    format!("https://steamcommunity.com/market/listings/753/{path}")
}

/// A game's cards on the Steam market: every one listed, normal and foil.
pub fn game_market_page(app_id: u32) -> String {
    format!(
        "https://steamcommunity.com/market/search?appid=753&category_753_Game%5B%5D=tag_app_{app_id}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cards_market_page_is_found_by_its_hash_name() {
        assert_eq!(
            market_page("960910-Madison"),
            "https://steamcommunity.com/market/listings/753/960910-Madison"
        );
        assert_eq!(
            market_page("1145360-Thanatos (Foil)"),
            "https://steamcommunity.com/market/listings/753/1145360-Thanatos%20%28Foil%29",
            "spaces and brackets are escaped"
        );
        assert_eq!(
            market_page("413410-Makoto Naegi & Mönokuma"),
            "https://steamcommunity.com/market/listings/753/413410-Makoto%20Naegi%20%26%20M%C3%B6nokuma",
            "and anything else byte by byte"
        );
        assert!(game_market_page(960_910).ends_with("tag_app_960910"));
    }
}
