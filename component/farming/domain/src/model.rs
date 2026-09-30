use std::{fmt, time::Duration};

use card::CardSets;
use chrono::{DateTime, Utc};
use game::SteamLibrary;
use session::{Mode, NewItem, Session, SetAside};

/// What the farmer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    /// Reading the library, or connecting.
    Checking,
    Farming,
    /// Another device is playing on the account; farming waits for it.
    Blocked,
    Error,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Idle => write!(f, "idle"),
            Status::Checking => write!(f, "checking"),
            Status::Farming => write!(f, "farming"),
            Status::Blocked => write!(f, "blocked"),
            Status::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FarmingStatus {
    pub status: Status,
    /// The library as the farmer sees it: hours counted as they're played,
    /// drops as they land.
    pub library: SteamLibrary,
    /// The card sets of the games looked at, with the copies that dropped
    /// since counted in.
    pub sets: CardSets,
    /// The games the farmer means to farm, by app ID, in the order it will.
    pub order: Vec<u32>,
    /// What's being played now.
    pub playing: Vec<u32>,
    /// How what's being played is farmed; `None` when nothing is.
    pub mode: Option<Mode>,
    /// While blocked: what the other device is playing, when Steam says.
    pub blocked_by: Option<u32>,
    /// When the farmer next looks at the cards; after an error, when it
    /// tries again.
    pub next_look: Option<DateTime<Utc>>,
    /// How often the game farmed alone has its cards looked at: every 5
    /// minutes on its last card, else every quarter of an hour. `None`
    /// when nothing is farmed alone.
    pub look_every: Option<Duration>,
    /// This session: every card that dropped, what was played, the games
    /// finished.
    pub session: Session,
    /// The games put behind the others after a long while without a drop.
    pub set_aside: Vec<SetAside>,
    pub note: String,
}

/// What a log line is about, so the UI can highlight the ones that matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventKind {
    #[default]
    Info,
    /// Routine looks at the cards.
    Progress,
    /// Started playing something.
    Playing,
    /// Moved on to something else before it was done.
    Switched,
    /// A card dropped.
    Dropped,
    /// A card that dropped, named a moment later.
    Identified,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FarmingEvent {
    pub kind: EventKind,
    pub message: String,
    pub status: Option<FarmingStatus>,
}

/// Something Steam said that the farmer acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    /// Another device started playing on the account (what, when Steam
    /// says): nothing played here counts until it stops.
    Blocked(Option<u32>),
    /// It stopped.
    Unblocked,
    /// Another device took over playing, and Steam signed this session off
    /// to let it: nothing played here counts until it stops.
    TakenOver,
    /// New items arrived: a card may have dropped. The items Steam listed,
    /// each once; none when it gave only a count.
    NewItems(Vec<NewItem>),
    /// Another session signed on in this one's place. Signing on again would
    /// knock that one off in turn.
    Replaced,
    /// The connection to Steam went, for this reason.
    Lost(String),
}
