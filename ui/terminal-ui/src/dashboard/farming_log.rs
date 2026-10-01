//! What the farmer reported, for the log: farming-words' sentence, and what
//! it's about, to highlight the ones that matter.

use farming::{FarmingEvent, Trouble};

use crate::dashboard::EventKind;

/// The log's line for what happened while farming, and what it's about.
pub(crate) fn line(event: &FarmingEvent) -> (EventKind, String) {
    let text = farming_words::event(event);
    let kind = match event {
        FarmingEvent::FarmingCards { .. } | FarmingEvent::BuildingHours { .. } => {
            EventKind::Playing
        }
        FarmingEvent::MovedOn { .. } | FarmingEvent::ChoicesChanged => EventKind::Switched,
        FarmingEvent::Dropped { .. } => EventKind::Dropped,
        FarmingEvent::Identified { .. } => EventKind::Identified,
        FarmingEvent::NewItems
        | FarmingEvent::Looked { .. }
        | FarmingEvent::Asking { .. }
        | FarmingEvent::ByCardPage { .. } => EventKind::Progress,
        FarmingEvent::HoursBuilt { .. }
        | FarmingEvent::AllDropped { .. }
        | FarmingEvent::PlayedElsewhere { .. }
        | FarmingEvent::TakenOver
        | FarmingEvent::ElsewhereStopped { .. } => EventKind::Info,
        FarmingEvent::Stalled { .. }
        | FarmingEvent::AskFailed { .. }
        | FarmingEvent::Untold { .. }
        | FarmingEvent::CardsUnread { .. }
        | FarmingEvent::FoilsUnread { .. } => EventKind::Warning,
        FarmingEvent::WentWrong { trouble, .. } => match trouble {
            Trouble::Lost(_) | Trouble::SignedOff => EventKind::Warning,
            Trouble::BadgesUnread(_) => EventKind::Error,
            // Farming stopped for good: the dashboard says how to start it.
            Trouble::Replaced => {
                return (EventKind::Error, format!("{text} Press p to start again."));
            }
        },
    };
    (kind, text)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn what_matters_most_stands_out() {
        let kind = |event| line(&event).0;
        assert_eq!(
            kind(FarmingEvent::FarmingCards {
                game: "Portal 2".into(),
                cards_left: 3
            }),
            EventKind::Playing
        );
        assert_eq!(
            kind(FarmingEvent::Dropped {
                game: "Portal 2".into(),
                count: 1,
                left: 2
            }),
            EventKind::Dropped
        );
        assert_eq!(
            kind(FarmingEvent::Looked {
                game: "Portal 2".into(),
                cards_left: 2
            }),
            EventKind::Progress
        );
        assert_eq!(
            kind(FarmingEvent::WentWrong {
                trouble: Trouble::Lost("the connection to Steam dropped".into()),
                again_in: Some(Duration::from_secs(60))
            }),
            EventKind::Warning
        );
    }

    #[test]
    fn farming_stopped_for_good_says_how_to_start_it_again() {
        let (kind, text) = line(&FarmingEvent::WentWrong {
            trouble: Trouble::Replaced,
            again_in: None,
        });
        assert_eq!(kind, EventKind::Error);
        assert!(text.starts_with("Another session signed in"));
        assert!(text.ends_with("knock it off. Press p to start again."));
    }
}
