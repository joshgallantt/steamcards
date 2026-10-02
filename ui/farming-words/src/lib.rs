//! Farming in words. The farmer says what happened, and where farming
//! stands, and keeping steamcards up to date what it found; these say it
//! as the user reads it, so the dashboard's log and the headless one tell
//! the same thing the same way.

mod counting;
mod event_words;
mod status_words;
mod update_words;

pub use event_words::event;
pub use status_words::{note, status};
pub use update_words::update;
