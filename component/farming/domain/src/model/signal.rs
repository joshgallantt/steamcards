use card::NewItem;
use game::{AppId, PlayingSignal};

/// Something Steam said that the farmer acts on: what the game component
/// hears of playing, a variant of [`PlayingSignal`] each, or new items, as
/// the card component hears of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Signal {
    Blocked(Option<AppId>),
    Unblocked,
    TakenOver,
    NewItems(Vec<NewItem>),
    Replaced,
    Lost(String),
}

impl From<PlayingSignal> for Signal {
    fn from(said: PlayingSignal) -> Self {
        match said {
            PlayingSignal::Blocked(by) => Signal::Blocked(by),
            PlayingSignal::Unblocked => Signal::Unblocked,
            PlayingSignal::TakenOver => Signal::TakenOver,
            PlayingSignal::Replaced => Signal::Replaced,
            PlayingSignal::Lost(why) => Signal::Lost(why),
        }
    }
}
