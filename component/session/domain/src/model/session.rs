use std::time::Duration;

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

impl Session {
    /// How long it has played by `now`: its stretches added up. Waiting for
    /// another device, and pauses, don't count.
    pub fn time_played(&self, now: DateTime<Utc>) -> Duration {
        self.stretches
            .iter()
            .map(|s| (s.to.unwrap_or(now) - s.from).to_std().unwrap_or_default())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use game::AppId;

    use super::*;
    use crate::Mode;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 2, h, m, 0).unwrap()
    }

    fn stretch(from: DateTime<Utc>, to: Option<DateTime<Utc>>) -> Stretch {
        Stretch {
            app_ids: vec![AppId(620)],
            mode: Mode::Cards,
            from,
            to,
        }
    }

    #[test]
    fn the_time_played_adds_up_the_stretches_and_leaves_out_the_waits() {
        let session = Session {
            started_at: at(9, 0),
            // Played 30 minutes, waited 20 for another device, and has
            // played 10 more by 10:00.
            stretches: vec![stretch(at(9, 0), Some(at(9, 30))), stretch(at(9, 50), None)],
            ..Session::default()
        };

        assert_eq!(session.time_played(at(10, 0)), Duration::from_secs(40 * 60));
        assert_eq!(Session::default().time_played(at(10, 0)), Duration::ZERO);
    }
}
