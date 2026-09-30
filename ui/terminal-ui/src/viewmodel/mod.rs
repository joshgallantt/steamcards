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
pub use onboarding::{NeedsAccount, Onboarding, Step};
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
