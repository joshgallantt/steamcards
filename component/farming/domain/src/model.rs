use std::{fmt, time::Duration};

use chrono::{DateTime, Utc};
use library::{CardAsset, SteamLibrary};

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
    /// How often the game farmed alone has its cards looked at: every 5
    /// minutes on its last card, else every quarter of an hour. `None`
    /// when nothing is farmed alone.
    pub look_every: Option<Duration>,
    /// This session: every card that dropped, what was played, the games
    /// finished.
    pub session: FarmingSession,
    /// The games put behind the others after a long while without a drop.
    pub set_aside: Vec<SetAside>,
    pub note: String,
}

/// One session of farming. It starts with the first run of farming and
/// carries on through pauses, until the user signs out or steamcards quits.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FarmingSession {
    /// When farming first started.
    pub started_at: DateTime<Utc>,
    /// Drops left across the library when the session first read it; `None`
    /// until it has.
    pub drops_left_at_start: Option<u32>,
    /// Games with drops left then.
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

/// What was played, how, and when: from when play started to when it
/// stopped, for any reason (done, switched, blocked, the connection lost,
/// paused, stopped). Waiting for another device, and being paused, aren't
/// stretches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stretch {
    pub app_ids: Vec<u32>,
    pub mode: Mode,
    pub from: DateTime<Utc>,
    /// `None` while it goes on.
    pub to: Option<DateTime<Utc>>,
}

/// One card that dropped. Each copy is its own drop: a look that finds two
/// new cards makes two.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Drop {
    /// When a look at the game, or a read of the library, found it.
    pub at: DateTime<Utc>,
    /// The game it dropped for.
    pub app_id: u32,
    pub card: DropCard,
    /// Which copy of that card (by name and border) the account then held:
    /// 1 the first, 2 or more a spare. `None` when the set wasn't known.
    pub copy: Option<u32>,
}

/// What's known of the card that dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropCard {
    /// Just found: which card it is is being found out.
    Identifying,
    /// The copy Steam described, by the item it announced.
    Identified(CardAsset),
    /// Named by the game's card page alone, whose count of it went up: no
    /// item to go with it, so never one to sell. The page read is the
    /// normal set's, so these aren't foils.
    NameOnly { name: String, foil: bool },
    /// Neither Steam nor the card page could tell.
    Unknown,
}

impl DropCard {
    /// The card's name, once it's known: "Madison".
    pub fn name(&self) -> Option<&str> {
        match self {
            DropCard::Identified(card) => Some(&card.name),
            DropCard::NameOnly { name, .. } => Some(name),
            DropCard::Identifying | DropCard::Unknown => None,
        }
    }

    /// Whether it's known to be a foil.
    pub fn is_foil(&self) -> bool {
        match self {
            DropCard::Identified(card) => card.foil,
            DropCard::NameOnly { foil, .. } => *foil,
            DropCard::Identifying | DropCard::Unknown => false,
        }
    }
}

impl Drop {
    /// A copy beyond the first of its card: a badge level takes one of each.
    pub fn is_spare(&self) -> bool {
        self.copy.is_some_and(|copy| copy > 1)
    }
}

/// A game whose last drop came this session: seen at `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finished {
    pub app_id: u32,
    pub at: DateTime<Utc>,
}

/// A game put behind the others after 10 hours without a drop: how often,
/// and when last.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetAside {
    pub app_id: u32,
    pub times: u8,
    pub since: DateTime<Utc>,
}

/// How long farming should take to finish, learnt from this session's drops
/// (see [`forecast`](crate::forecast)).
#[derive(Debug, Clone, PartialEq)]
pub struct Forecast {
    /// The time to finish, if left farming.
    pub eta: Duration,
    /// Where it falls 80% of the time; `None` before the second drop.
    pub band: Option<(Duration, Duration)>,
    /// Too few drops to learn from yet: it assumes 30 minutes a drop.
    pub assumed: bool,
    /// The part of `eta` spent building hours for games short of 3.
    pub hours_term: Duration,
    /// Drops an hour, farming alone.
    pub rate: f64,
    /// When each game's last card should drop, by app ID, counted from now,
    /// in the order they're farmed.
    pub per_game: Vec<(u32, Duration)>,
    pub made_at: DateTime<Utc>,
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
    /// New items arrived: a card may have dropped. The items Steam listed,
    /// each once; none when it gave only a count.
    NewItems(Vec<NewItem>),
    /// Another session signed on in this one's place. Signing on again would
    /// knock that one off in turn.
    Replaced,
    /// The connection to Steam went, for this reason.
    Lost(String),
}

/// An item Steam announced as new in the account's inventory: perhaps a
/// card that just dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NewItem {
    /// Its ID in the inventory, which says which card it is.
    pub asset_id: u64,
    /// The game it came from, when Steam says.
    pub app_id: Option<u32>,
    /// When it arrived, when Steam says.
    pub gained_at: Option<DateTime<Utc>>,
}
