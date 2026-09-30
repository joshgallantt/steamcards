// Numbers, money and time as the screens write them (docs/design/ui.md
// §5.2). Pure: a figure goes in and its words come out, so every example the
// spec gives is a test below. Times are shown on the local clock, in the zone
// the screen is given, never in UTC.

use std::time::Duration;

use chrono::{DateTime, FixedOffset, NaiveTime, TimeDelta, Utc};
use market::{Estimate, Held, Money};

const MINUTE: f64 = 60.0;
const HOUR: f64 = 60.0 * MINUTE;
const DAY: f64 = 24.0 * HOUR;

// ── Counts ───────────────────────────────────────────────────────────────────

/// Drops in prose: "3 of 4".
pub(crate) fn of(k: u32, n: u32) -> String {
    format!("{k} of {n}")
}

/// Drops in a table: "3/4".
pub(crate) fn slash(k: u32, n: u32) -> String {
    format!("{k}/{n}")
}

/// "1 to go".
pub(crate) fn to_go(n: u32) -> String {
    format!("{n} to go")
}

/// "1 card", "5 cards".
pub(crate) fn cards(n: usize) -> String {
    plural(n, "card", "cards")
}

/// "1 spare", "2 spares".
pub(crate) fn spares(n: u32) -> String {
    plural(n as usize, "spare", "spares")
}

/// "1 game", "57 games".
pub(crate) fn games(n: usize) -> String {
    plural(n, "game", "games")
}

/// "1 drop", "236 drops".
pub(crate) fn drops(n: u32) -> String {
    plural(n as usize, "drop", "drops")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// A set as a badge sees it: the distinct cards held of its size, and the
/// spares beyond one of each. "2 of 5 cards · 1 spare".
pub(crate) fn the_set(have: usize, size: usize, spare: u32) -> String {
    match spare {
        0 => format!("{have} of {size} cards"),
        n => format!("{have} of {size} cards · {}", spares(n)),
    }
}

/// How many of a card the account holds: "×2", "×1", or "—" for none, never
/// a tick.
pub(crate) fn count(owned: u32) -> String {
    match owned {
        0 => "—".to_owned(),
        n => format!("×{n}"),
    }
}

/// Hours on record: one decimal under 10, whole above. "4.0h", "16h",
/// "350h", "1,234h".
pub(crate) fn hours(h: f64) -> String {
    if h < 10.0 {
        format!("{h:.1}h")
    } else {
        format!("{}h", grouped(h.round_ties_even() as u64))
    }
}

/// "1,234".
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Whole, rounded down, beside a gauge only: "6%", "43%".
pub(crate) fn percent(k: u32, n: u32) -> String {
    let p = if n == 0 {
        0
    } else {
        u64::from(k) * 100 / u64::from(n)
    };
    format!("{p}%")
}

/// Drops an hour, one decimal: "2.1 drops an hour".
pub(crate) fn rate(per_hour: f64) -> String {
    format!("{per_hour:.1} drops an hour")
}

// ── Money ────────────────────────────────────────────────────────────────────

/// An amount in the wallet's currency, grouped: "£0.05", "£1,234.50".
pub(crate) fn money(m: Money) -> String {
    m.grouped()
}

/// What cards held are worth, at least: "≥ £1.45" while some aren't priced,
/// "£1.45" when all are.
pub(crate) fn at_least(held: &Held) -> String {
    if held.unpriced > 0 {
        format!("≥ {}", money(held.total))
    } else {
        money(held.total)
    }
}

/// Cards held, with how many aren't priced: "≥ £1.45 · 3 unpriced", or
/// "£1.45" when all are.
pub(crate) fn held(held: &Held) -> String {
    if held.unpriced > 0 {
        format!("{} · {} unpriced", at_least(held), held.unpriced)
    } else {
        at_least(held)
    }
}

/// An estimate: "≈ £16.79".
pub(crate) fn about(m: Money) -> String {
    format!("≈ {}", money(m))
}

/// A partial estimate, and what it leaves out: "≈ £2.37 so far · 48
/// unpriced".
pub(crate) fn so_far(e: &Estimate) -> String {
    format!("{} so far · {} unpriced", about(e.value), e.unpriced_games)
}

// ── Time ─────────────────────────────────────────────────────────────────────

/// Minutes, rounded as the spec's generator rounds them: half to even.
fn minutes(d: Duration) -> f64 {
    d.as_secs_f64() / MINUTE
}

/// A duration in two units at most: "<1m", "38m", "8h 17m", "4d 21h".
pub(crate) fn duration(d: Duration) -> String {
    whole_minutes(minutes(d).round_ties_even() as u64)
}

fn whole_minutes(m: u64) -> String {
    match m {
        0 => "<1m".to_owned(),
        m if m < 60 => format!("{m}m"),
        m if m < 24 * 60 => match (m / 60, m % 60) {
            (h, 0) => format!("{h}h"),
            (h, mm) => format!("{h}h {mm}m"),
        },
        m => {
            let (mut days, rest) = (m / (24 * 60), m % (24 * 60));
            let mut h = (rest as f64 / 60.0).round_ties_even() as u64;
            if h == 24 {
                days += 1;
                h = 0;
            }
            match h {
                0 => format!("{days}d"),
                h => format!("{days}d {h}h"),
            }
        }
    }
}

/// How long until `at`: minutes, and seconds only under a minute. "in 4m",
/// "in 42s", "now".
pub(crate) fn countdown(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (at - now).num_seconds();
    match secs {
        s if s <= 0 => "now".to_owned(),
        s if s < 60 => format!("in {s}s"),
        s => format!("in {}", duration(Duration::from_secs(s.unsigned_abs()))),
    }
}

/// How old a price is, in its largest unit: "8h", "2h", "40m", "3d".
pub(crate) fn age(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs >= DAY {
        format!("{}d", (secs / DAY) as u64)
    } else if secs >= HOUR {
        format!("{}h", (secs / HOUR) as u64)
    } else if secs >= MINUTE {
        format!("{}m", (secs / MINUTE) as u64)
    } else {
        "<1m".to_owned()
    }
}

/// "2h ago".
pub(crate) fn ago(d: Duration) -> String {
    format!("{} ago", age(d))
}

/// The step an estimate this far off is rounded to, in minutes: 5 under an
/// hour, 30 under 10 hours, an hour under 2 days, 3 hours beyond (§5.2).
fn step(minutes: f64) -> f64 {
    if minutes < 60.0 {
        5.0
    } else if minutes < 10.0 * 60.0 {
        30.0
    } else if minutes < 48.0 * 60.0 {
        60.0
    } else {
        180.0
    }
}

/// A time to go that's an estimate, no sharper than its step: "25m",
/// "1h 30m", "11h", "1d 12h", "4d 21h".
pub(crate) fn estimate(d: Duration) -> String {
    let m = minutes(d);
    let s = step(m);
    whole_minutes((s * (m / s).round_ties_even()).max(s) as u64)
}

/// The time to finish: "≈ 4d 21h".
pub(crate) fn eta(d: Duration) -> String {
    format!("≈ {}", estimate(d))
}

/// Where the time to finish falls 80% of the time: "80%: 3d 13h – 6d 15h".
pub(crate) fn band((low, high): (Duration, Duration)) -> String {
    format!("80%: {} – {}", duration(low), duration(high))
}

/// An estimate `d` from `now` as a clock time, rounded the same way: "17:55",
/// "19:00", "Wed 05:00", or a day alone beyond 2 days, "Thu". No estimate
/// shows a clock minute days ahead.
pub(crate) fn estimate_at(now: DateTime<Utc>, d: Duration, zone: FixedOffset) -> String {
    let m = minutes(d);
    let s = step(m);
    let local = now.with_timezone(&zone);
    let midnight = local.with_time(NaiveTime::MIN).single().unwrap_or(local);
    let since_midnight = (local - midnight).num_seconds() as f64 / MINUTE;
    let at = s * ((since_midnight + m) / s).round_ties_even();
    let at = midnight + TimeDelta::minutes(at as i64);
    if m >= 48.0 * 60.0 {
        at.format("%a").to_string()
    } else if at.date_naive() == local.date_naive() {
        at.format("%H:%M").to_string()
    } else {
        at.format("%a %H:%M").to_string()
    }
}

/// A time on the local clock, 24-hour, with the day when it isn't today:
/// "17:23", "Sat 21:50".
pub(crate) fn clock(at: DateTime<Utc>, now: DateTime<Utc>, zone: FixedOffset) -> String {
    let (at, now) = (at.with_timezone(&zone), now.with_timezone(&zone));
    if at.date_naive() == now.date_naive() {
        at.format("%H:%M").to_string()
    } else {
        at.format("%a %H:%M").to_string()
    }
}

/// A time on the local clock with its day, whatever day it is: "Sun 17:44".
pub(crate) fn day_and_time(at: DateTime<Utc>, zone: FixedOffset) -> String {
    at.with_timezone(&zone).format("%a %H:%M").to_string()
}

/// A day on the local calendar: "Sun 4 Oct".
pub(crate) fn date(at: DateTime<Utc>, zone: FixedOffset) -> String {
    at.with_timezone(&zone).format("%a %-d %b").to_string()
}

/// A weekday on the local calendar: "Tue".
pub(crate) fn weekday(at: DateTime<Utc>, zone: FixedOffset) -> String {
    at.with_timezone(&zone).format("%a").to_string()
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use market::{Basis, Currency};

    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    fn mins(m: f64) -> Duration {
        Duration::from_secs_f64(m * 60.0)
    }

    fn pounds(minor: i64) -> Money {
        Money::new(minor, Currency::GBP)
    }

    /// British Summer Time, an hour ahead of UTC: a screen that showed UTC
    /// would read an hour early.
    fn bst() -> FixedOffset {
        FixedOffset::east_opt(3600).unwrap()
    }

    /// Tue 29 Sep 2026, 17:31 on the local clock.
    fn now() -> DateTime<Utc> {
        bst()
            .with_ymd_and_hms(2026, 9, 29, 17, 31, 0)
            .unwrap()
            .to_utc()
    }

    #[test]
    fn drops_read_k_of_n_in_prose_and_k_slash_n_in_tables() {
        assert_eq!(of(3, 4), "3 of 4");
        assert_eq!(slash(3, 4), "3/4");
        assert_eq!(slash(6, 11), "6/11");
        assert_eq!(to_go(1), "1 to go");
        assert_eq!(drops(236), "236 drops");
        assert_eq!(drops(1), "1 drop");
        assert_eq!(games(57), "57 games");
        assert_eq!(games(1), "1 game");
        assert_eq!(cards(5), "5 cards");
        assert_eq!(cards(1), "1 card");
    }

    #[test]
    fn the_set_counts_distinct_cards_and_spares() {
        assert_eq!(the_set(2, 5, 1), "2 of 5 cards · 1 spare");
        assert_eq!(the_set(4, 6, 2), "4 of 6 cards · 2 spares");
        assert_eq!(the_set(1, 7, 0), "1 of 7 cards");
        assert_eq!(spares(1), "1 spare");
    }

    #[test]
    fn a_cards_count_is_never_a_tick() {
        assert_eq!(count(2), "×2");
        assert_eq!(count(1), "×1");
        assert_eq!(count(0), "—");
    }

    #[test]
    fn hours_have_one_decimal_under_ten() {
        assert_eq!(hours(4.0), "4.0h");
        assert_eq!(hours(3.4), "3.4h");
        assert_eq!(hours(0.0), "0.0h");
        assert_eq!(hours(16.2), "16h");
        assert_eq!(hours(350.0), "350h");
        assert_eq!(hours(1_234.0), "1,234h");
    }

    #[test]
    fn money_is_the_wallets_grouped() {
        assert_eq!(money(pounds(5)), "£0.05");
        assert_eq!(money(pounds(145)), "£1.45");
        assert_eq!(money(pounds(123_450)), "£1,234.50");
    }

    fn held_of(total: i64, unpriced: u32) -> Held {
        Held {
            total: pounds(total),
            priced: 13,
            unpriced,
            not_marketable: 0,
            oldest: None,
        }
    }

    #[test]
    fn cards_held_say_at_least_while_some_arent_priced() {
        assert_eq!(held(&held_of(145, 3)), "≥ £1.45 · 3 unpriced");
        assert_eq!(held(&held_of(145, 0)), "£1.45");
        assert_eq!(at_least(&held_of(114, 3)), "≥ £1.14");
        assert_eq!(at_least(&held_of(74, 0)), "£0.74");
    }

    #[test]
    fn estimates_say_about_and_what_they_leave_out() {
        assert_eq!(about(pounds(1_679)), "≈ £16.79");
        let partial = Estimate {
            value: pounds(237),
            excl_foils: true,
            unpriced_games: 48,
            basis: Basis::List,
        };
        assert_eq!(so_far(&partial), "≈ £2.37 so far · 48 unpriced");
    }

    #[test]
    fn durations_have_two_units_at_most() {
        assert_eq!(duration(Duration::from_secs(20)), "<1m");
        assert_eq!(duration(38 * MIN), "38m");
        assert_eq!(duration(8 * 60 * MIN + 17 * MIN), "8h 17m");
        assert_eq!(duration(7 * 60 * MIN + 40 * MIN), "7h 40m");
        assert_eq!(duration(3 * 60 * MIN), "3h");
        assert_eq!(duration((4 * 24 + 21) * 60 * MIN), "4d 21h");
        assert_eq!(duration((3 * 24 + 13) * 60 * MIN + 20 * MIN), "3d 13h");
        assert_eq!(
            duration((2 * 24 + 23) * 60 * MIN + 45 * MIN),
            "3d",
            "to the hour"
        );
        assert_eq!(duration(2 * 24 * 60 * MIN), "2d");
    }

    #[test]
    fn countdowns_are_minutes_and_seconds_only_under_a_minute() {
        let at = |secs: i64| now() + TimeDelta::seconds(secs);
        assert_eq!(countdown(at(4 * 60), now()), "in 4m");
        assert_eq!(countdown(at(42), now()), "in 42s");
        assert_eq!(countdown(at(0), now()), "now");
        assert_eq!(countdown(at(-5), now()), "now");
        assert_eq!(countdown(at(65 * 60), now()), "in 1h 5m");
    }

    #[test]
    fn the_time_to_finish_has_its_band() {
        assert_eq!(eta((4 * 24 + 21) * 60 * MIN), "≈ 4d 21h");
        assert_eq!(eta((5 * 24 + 9) * 60 * MIN), "≈ 5d 9h");
        let band_of = ((3 * 24 + 13) * 60 * MIN, (6 * 24 + 15) * 60 * MIN);
        assert_eq!(band(band_of), "80%: 3d 13h – 6d 15h");
        assert_eq!(
            date(now() + (4 * 24 + 21) * 60 * MIN, bst()),
            "Sun 4 Oct",
            "around Sun 4 Oct"
        );
    }

    #[test]
    fn estimates_are_no_sharper_than_their_step() {
        // The data set's schedule: Heavy Rain, LIMBO, Oxygen Not Included,
        // We Were Here Expeditions, Days Gone, TCG Card Shop Simulator,
        // Witch It, The Binding of Isaac, Oddworld and Warframe.
        let cases = [
            (24.04, "25m"),
            (82.04, "1h 30m"),
            (140.04, "2h 30m"),
            (227.05, "4h"),
            (662.06, "11h"),
            (1_503.09, "1d 1h"),
            (2_147.81, "1d 12h"),
            (2_930.84, "2d"),
            (3_104.85, "2d 3h"),
            (5_743.95, "4d"),
            (7_020.0, "4d 21h"),
            (1.0, "5m"),
        ];
        for (m, said) in cases {
            assert_eq!(estimate(mins(m)), said, "{m} minutes");
        }
    }

    #[test]
    fn estimates_as_clock_times_have_the_day_when_it_isnt_today() {
        let at = |m: f64| estimate_at(now(), mins(m), bst());
        assert_eq!(at(24.04), "17:55");
        assert_eq!(at(82.04), "19:00");
        assert_eq!(at(140.04), "20:00");
        assert_eq!(at(227.05), "21:30");
        assert_eq!(at(662.06), "Wed 05:00");
        assert_eq!(at(3_104.85), "Thu", "a day alone beyond 2 days");
        assert_eq!(at(7_020.0), "Sun");
    }

    #[test]
    fn clock_times_are_local_with_the_day_when_it_isnt_today() {
        let zone = bst();
        let today = zone
            .with_ymd_and_hms(2026, 9, 29, 17, 23, 0)
            .unwrap()
            .to_utc();
        assert_eq!(clock(today, now(), zone), "17:23");
        let saturday = zone
            .with_ymd_and_hms(2026, 10, 3, 21, 50, 0)
            .unwrap()
            .to_utc();
        let sunday = zone
            .with_ymd_and_hms(2026, 10, 4, 17, 44, 0)
            .unwrap()
            .to_utc();
        assert_eq!(clock(saturday, sunday, zone), "Sat 21:50");
        assert_eq!(day_and_time(sunday, zone), "Sun 17:44");
        assert_eq!(weekday(now(), zone), "Tue");
        let utc = FixedOffset::east_opt(0).unwrap();
        assert_eq!(clock(today, now(), utc), "16:23", "the zone it's given");
    }

    #[test]
    fn percentages_round_down() {
        assert_eq!(percent(16, 252), "6%");
        assert_eq!(percent(183, 421), "43%");
        assert_eq!(percent(167, 421), "39%");
        assert_eq!(percent(0, 0), "0%");
    }

    #[test]
    fn rates_have_one_decimal() {
        assert_eq!(rate(18.0 / (1.0 + 7.0 + 40.0 / 60.0)), "2.1 drops an hour");
        assert_eq!(rate(2.0), "2.0 drops an hour");
    }

    #[test]
    fn a_prices_age_is_its_largest_unit() {
        assert_eq!(age(8 * 60 * MIN + 3 * MIN), "8h");
        assert_eq!(ago(2 * 60 * MIN + 10 * MIN), "2h ago");
        assert_eq!(age(40 * MIN), "40m");
        assert_eq!(age(3 * 24 * 60 * MIN), "3d");
        assert_eq!(age(Duration::from_secs(5)), "<1m");
    }
}
