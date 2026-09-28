use std::fmt;

use chrono::{DateTime, Utc};

/// A game with trading cards, as its badge shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub app_id: u32,
    pub name: String,
    /// Hours on record.
    pub hours: f64,
    /// Card drops still to come from playing it.
    pub cards_left: u32,
    /// Card drops so far.
    pub cards_dropped: u32,
}

impl Game {
    /// Every card that playing it drops has dropped.
    pub fn is_done(&self) -> bool {
        self.cards_left == 0
    }

    /// Every card playing it drops, dropped or not.
    pub fn cards_total(&self) -> u32 {
        self.cards_left + self.cards_dropped
    }
}

/// What the farmer is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Status {
    #[default]
    Idle,
    /// Reading the badges, or connecting.
    Checking,
    Farming,
    /// Another device is playing on this account; farming waits for it.
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
    /// One game at a time, until its cards have dropped.
    Cards,
    /// Several at once, building up hours before their cards start to drop.
    Hours,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FarmingStatus {
    pub status: Status,
    /// What's being played now.
    pub playing: Vec<u32>,
    /// How what's being played is farmed; `None` when nothing is.
    pub mode: Option<Mode>,
    /// Every game with cards, in the order they'll be farmed; finished and
    /// unwanted ones after.
    pub games: Vec<Game>,
    /// While blocked: what the other device is playing, when Steam says.
    pub blocked_by: Option<u32>,
    /// When the farmer next looks at the cards.
    pub next_check: Option<DateTime<Utc>>,
    pub note: String,
}

/// What a log line is about, so the UI can highlight the ones that matter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EventKind {
    #[default]
    Info,
    /// Routine checks.
    Progress,
    /// Started playing something.
    Playing,
    /// Moved on to something else.
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
    /// Another device started playing on this account (what, when Steam
    /// says): nothing here counts as played until it stops.
    Blocked(Option<u32>),
    /// It stopped.
    Unblocked,
    /// New items arrived: a card may have dropped.
    NewItems,
    /// The connection to Steam went, for this reason.
    Lost(String),
}

/// Every game with cards, for browsing, whether farming has started or not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Library {
    pub games: Vec<Game>,
    /// Why they couldn't be listed, when they couldn't.
    pub failed: Option<String>,
}
