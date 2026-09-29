use std::fmt;

use chrono::{DateTime, Utc};
use library::SteamLibrary;

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

/// How the games being played are farmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// One game on its own, until its cards have dropped.
    Cards,
    /// Several together, building up hours until their cards can drop.
    Hours,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FarmingStatus {
    pub status: Status,
    /// The library as the farmer sees it: hours counted as they're played,
    /// drops as they land.
    pub library: SteamLibrary,
    /// The games the farmer means to farm, by app ID, in the order it will.
    pub order: Vec<u32>,
    /// What's being played now.
    pub playing: Vec<u32>,
    /// How what's being played is farmed; `None` when nothing is.
    pub mode: Option<Mode>,
    /// While blocked: what the other device is playing, when Steam says.
    pub blocked_by: Option<u32>,
    /// When the farmer next looks at the cards.
    pub next_look: Option<DateTime<Utc>>,
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
    /// New items arrived: a card may have dropped.
    NewItems,
    /// Another session signed on in this one's place. Signing on again would
    /// knock that one off in turn.
    Replaced,
    /// The connection to Steam went, for this reason.
    Lost(String),
}
