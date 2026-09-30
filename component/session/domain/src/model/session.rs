use chrono::{DateTime, Utc};

use crate::{Drop, Finished, Forecast, Stretch};

/// One session of farming. It starts with the first run of farming and
/// carries on through pauses, until the user signs out or steamcards quits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Session {
    /// When farming first started.
    pub started_at: DateTime<Utc>,
    /// Drops left in the games it farms, as the farm order stood when the
    /// session first read the library: skipped games and sale badges are
    /// left out. `None` until it has.
    pub drops_left_at_start: Option<u32>,
    /// How many games it farms then.
    pub games_at_start: Option<u32>,
    /// Every card that dropped, in the order they were found. Each copy is
    /// a drop of its own.
    pub drops: Vec<Drop>,
    /// What was played, how, and when.
    pub stretches: Vec<Stretch>,
    /// The games whose last drop came this session.
    pub finished: Vec<Finished>,
    /// The first forecast with two drops farming alone to learn from.
    pub first_forecast: Option<Forecast>,
}
