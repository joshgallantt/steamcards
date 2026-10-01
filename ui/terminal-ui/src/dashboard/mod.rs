//! The dashboard: what farming is doing, this session's cards, the games in
//! the order they're farmed, and what it's all worth. Its view models, the
//! views that draw it, and the log and a game's details as pop-ups.

mod card_page;
pub(crate) mod dashboard_view;
pub(crate) mod detail_view;
mod event_kind;
pub(crate) mod farming_log;
mod farming_view_model;
mod library_view_model;
mod log_entry;
pub(crate) mod log_view;
mod market_view_model;
pub(crate) mod price_words;
mod queue;
mod summary;

pub use card_page::{card_page, open_in_browser};
pub(crate) use event_kind::EventKind;
pub use farming_view_model::FarmingViewModel;
pub use library_view_model::LibraryViewModel;
pub(crate) use log_entry::LogEntry;
pub use market_view_model::MarketViewModel;
pub use queue::{Queue, QueueEntry, Section};
pub use summary::{
    CardName, CardPrice, SessionCard, Summary, Value, card_price, prices_wanted, session_cards,
    value_to_come,
};
