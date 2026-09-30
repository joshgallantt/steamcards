// Presentation-layer view models. Depend only on domain use cases and entities —
// never on repositories, data sources, or any data-layer type.

mod account;
mod farming;
mod games;
mod library;
mod login;
mod market;
mod onboarding;
mod queue;
mod summary;

pub use account::Account;
pub use farming::Farming;
pub use games::{GameRow, Games};
pub use library::Library;
pub use login::{Login, LoginUpdate};
pub use market::Market;
pub use onboarding::{NeedsAccount, Onboarding, Step};
pub use queue::{Queue, QueueEntry, Section};
pub use summary::{
    CardName, CardPrice, SessionCard, Summary, Value, card_price, prices_wanted, session_cards,
    value_to_come,
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
