//! Counts and durations as the sentences say them.

use std::time::Duration;

/// "1 card", "3 cards".
pub(crate) fn cards(n: u32) -> String {
    if n == 1 {
        "1 card".into()
    } else {
        format!("{n} cards")
    }
}

/// "a minute", "5 minutes": how long, to the minute, rounding up.
pub(crate) fn minutes(d: Duration) -> String {
    match d.as_secs().div_ceil(60) {
        0 | 1 => "a minute".into(),
        n => format!("{n} minutes"),
    }
}

/// "an hour", "10 hours": how long, in whole hours.
pub(crate) fn hours(d: Duration) -> String {
    match d.as_secs() / (60 * 60) {
        1 => "an hour".into(),
        n => format!("{n} hours"),
    }
}

/// "2nd", "3rd", "11th", "21st".
pub(crate) fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: Duration = Duration::from_secs(60);

    #[test]
    fn copies_are_counted_as_people_say_them() {
        let said: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 101, 111]
            .into_iter()
            .map(ordinal)
            .collect();
        assert_eq!(
            said,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd",
                "101st", "111th"
            ]
        );
    }

    #[test]
    fn waits_are_said_to_the_minute_rounding_up() {
        assert_eq!(minutes(Duration::ZERO), "a minute");
        assert_eq!(minutes(MINUTE), "a minute");
        assert_eq!(minutes(4 * MINUTE + Duration::from_secs(1)), "5 minutes");
        assert_eq!(hours(60 * MINUTE), "an hour");
        assert_eq!(hours(10 * 60 * MINUTE), "10 hours");
    }
}
