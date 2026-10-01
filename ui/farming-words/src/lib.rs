//! Farming in words. The farmer says what happened, and where farming
//! stands; these say it as the user reads it, so the dashboard's log and
//! the headless one tell the same thing the same way.

mod counting;
mod event_words;
mod status_words;

pub use event_words::event;
pub use status_words::{note, status};
