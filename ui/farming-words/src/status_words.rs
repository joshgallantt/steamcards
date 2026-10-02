//! What the status line says: what the farmer is doing, in a word, and
//! beside it why, or for how long.

use chrono::{DateTime, Utc};
use farming::{FarmingStatus, NothingToFarm, Status, Trouble};

use crate::{counting::minutes, event_words::SIGNED_OFF};

/// What the farmer is doing, in a word.
pub fn status(status: Status) -> &'static str {
    match status {
        Status::Idle => "idle",
        Status::Checking => "checking",
        Status::Farming => "farming",
        Status::Blocked => "blocked",
        Status::Error => "error",
    }
}

/// What the status says beside that word at `now`, if anything: why
/// there's nothing to farm, how long farming waits, or what went wrong.
pub fn note(s: &FarmingStatus, now: DateTime<Utc>) -> Option<String> {
    let wait = s
        .next_look
        .map(|at| minutes((at - now).to_std().unwrap_or_default()));
    match s.status {
        Status::Farming => None,
        Status::Checking => Some("reading your badges…".into()),
        Status::Idle => s.nothing_to_farm.map(|why| nothing_to_farm(why).into()),
        Status::Blocked => Some(match wait {
            Some(wait) => format!("carrying on in {wait}…"),
            None => "playing on another device — farming waits until it stops".into(),
        }),
        Status::Error => s.trouble.as_ref().map(|trouble| match trouble {
            Trouble::BadgesUnread(_) => match wait {
                Some(wait) => format!("couldn't read your badges — trying again in {wait}"),
                None => "couldn't read your badges".into(),
            },
            Trouble::Lost(why) => why.clone(),
            Trouble::SignedOff => SIGNED_OFF.into(),
            Trouble::Replaced => "stopped: another session took over".into(),
        }),
    }
}

fn nothing_to_farm(why: NothingToFarm) -> &'static str {
    match why {
        NothingToFarm::NoGames => "no games with trading cards on this account",
        NothingToFarm::AllDropped => "every card has dropped",
        NothingToFarm::PrioritiesDone => {
            "\"only priority\" is on, and your priority games are done"
        }
        NothingToFarm::AllSkipped => "every game with cards left is skipped",
        NothingToFarm::HeldBack {
            private,
            refundable_until,
        } => match (private, refundable_until.is_some()) {
            (true, false) => "the games left are private: Steam drops no cards for them",
            (false, true) => "the games left can still be refunded: they're farmed once they can't",
            (true, true) => "the games left are private, or can still be refunded",
            (false, false) => "every game with cards left is skipped",
        },
        NothingToFarm::NotDropping => "Steam isn't dropping cards for the games left",
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;

    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-09-29T09:14:00Z")
            .unwrap()
            .to_utc()
    }

    fn noted(status: Status, change: impl FnOnce(&mut FarmingStatus)) -> Option<String> {
        let mut s = FarmingStatus {
            status,
            ..Default::default()
        };
        change(&mut s);
        note(&s, now())
    }

    #[test]
    fn idle_says_why_theres_nothing_to_farm() {
        let idle = |why| noted(Status::Idle, |s| s.nothing_to_farm = Some(why)).unwrap_or_default();
        assert_eq!(
            idle(NothingToFarm::NoGames),
            "no games with trading cards on this account"
        );
        assert_eq!(idle(NothingToFarm::AllDropped), "every card has dropped");
        assert_eq!(
            idle(NothingToFarm::PrioritiesDone),
            "\"only priority\" is on, and your priority games are done"
        );
        assert_eq!(
            idle(NothingToFarm::AllSkipped),
            "every game with cards left is skipped"
        );
        assert_eq!(
            idle(NothingToFarm::NotDropping),
            "Steam isn't dropping cards for the games left"
        );
        let held = |private, refundable: bool| {
            idle(NothingToFarm::HeldBack {
                private,
                refundable_until: refundable.then(now),
            })
        };
        assert_eq!(
            held(true, false),
            "the games left are private: Steam drops no cards for them"
        );
        assert_eq!(
            held(false, true),
            "the games left can still be refunded: they're farmed once they can't"
        );
        assert_eq!(
            held(true, true),
            "the games left are private, or can still be refunded"
        );
    }

    #[test]
    fn waiting_for_another_device_says_how_long_once_its_done() {
        assert_eq!(
            noted(Status::Blocked, |_| {}).as_deref(),
            Some("playing on another device — farming waits until it stops")
        );
        assert_eq!(
            noted(Status::Blocked, |s| s.next_look =
                Some(now() + TimeDelta::seconds(59)))
            .as_deref(),
            Some("carrying on in a minute…")
        );
        assert_eq!(
            noted(Status::Blocked, |s| s.next_look =
                Some(now() + TimeDelta::minutes(5)))
            .as_deref(),
            Some("carrying on in 5 minutes…")
        );
    }

    #[test]
    fn an_error_says_what_went_wrong() {
        let error = |trouble, again: Option<TimeDelta>| {
            noted(Status::Error, |s| {
                s.trouble = Some(trouble);
                s.next_look = again.map(|wait| now() + wait);
            })
            .unwrap_or_default()
        };
        assert_eq!(
            error(
                Trouble::BadgesUnread("steamcommunity.com didn't answer".into()),
                Some(TimeDelta::minutes(5))
            ),
            "couldn't read your badges — trying again in 5 minutes"
        );
        assert_eq!(
            error(
                Trouble::Lost("the connection to Steam dropped".into()),
                Some(TimeDelta::minutes(1))
            ),
            "the connection to Steam dropped"
        );
        assert_eq!(
            error(Trouble::SignedOff, Some(TimeDelta::minutes(1))),
            "Steam signed this session off"
        );
        assert_eq!(
            error(Trouble::Replaced, None),
            "stopped: another session took over"
        );
    }

    #[test]
    fn farming_needs_no_note_and_checking_says_what_it_reads() {
        assert_eq!(noted(Status::Farming, |_| {}), None);
        assert_eq!(
            noted(Status::Checking, |_| {}).as_deref(),
            Some("reading your badges…")
        );
        let words: Vec<&str> = [
            Status::Idle,
            Status::Checking,
            Status::Farming,
            Status::Blocked,
            Status::Error,
        ]
        .into_iter()
        .map(status)
        .collect();
        assert_eq!(words, ["idle", "checking", "farming", "blocked", "error"]);
    }
}
