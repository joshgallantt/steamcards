//! The numbers keeping up to date runs on.

use std::time::Duration;

/// How often to look for a newer release while steamcards runs: once a day,
/// as the GitHub CLI does.
pub(crate) const LOOK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
