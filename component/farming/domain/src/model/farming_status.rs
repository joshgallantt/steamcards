use std::time::Duration;

use card::CardSets;
use chrono::{DateTime, Utc};
use game::SteamLibrary;
use session::{Mode, Session, SetAside};

use crate::Status;

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
