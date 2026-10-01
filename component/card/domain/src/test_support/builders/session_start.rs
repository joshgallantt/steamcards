use chrono::{DateTime, Utc};

/// Tuesday 29 September 2026, 09:14: when the session in the design's
/// mockups starts.
pub fn session_start() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-09-29T09:14:00Z")
        .expect("a valid date")
        .to_utc()
}
