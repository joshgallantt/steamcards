use std::time::Duration;

use crate::Trouble;

/// Something that happened while farming, with what's needed to tell it:
/// the screens choose the words. Games go by their names, as the farmer
/// knew them then.
#[derive(Debug, Clone, PartialEq)]
pub enum FarmingEvent {
    /// Started farming a game on its own, with this many cards still to
    /// drop.
    FarmingCards { game: String, cards_left: u32 },
    /// Started playing games together, `games` of them, until the first,
    /// `lead`, has the hours its cards need.
    BuildingHours { lead: String, games: usize },
    /// A game played for its hours has them now: its cards can drop.
    HoursBuilt { game: String },
    /// Steam says new items arrived: a card may have dropped.
    NewItems,
    /// A look at a game's cards found no new drop.
    Looked { game: String, cards_left: u32 },
    /// Cards dropped for a game, `count` at once, with `left` still to drop.
    /// Which they were is told after.
    Dropped {
        game: String,
        count: usize,
        left: u32,
    },
    /// Every card has dropped for a game.
    AllDropped { game: String },
    /// A game dropped nothing for `after`: it waits behind the others, or,
    /// set aside too often, is left be `for_good`.
    Stalled {
        game: String,
        after: Duration,
        for_good: bool,
    },
    /// The user's choices changed, and farming moved on from a game:
    /// something is ranked above it now, or it isn't to be farmed.
    MovedOn { game: String, outranked: bool },
    /// The user's choices changed while building hours: farming starts
    /// again, in the new order.
    ChoicesChanged,
    /// Steam is asked which cards these items are, `items` of them.
    Asking { items: usize },
    /// Steam couldn't say which cards they were: the card page may.
    AskFailed { why: String },
    /// Steam didn't say which card dropped for a game: its card page tells.
    ByCardPage { game: String },
    /// A card that dropped, named: and which copy of it this is, when
    /// that's known.
    Identified {
        game: String,
        card: String,
        foil: bool,
        copy: Option<u32>,
    },
    /// A card dropped that nothing could name.
    Untold { game: String },
    /// A game's cards couldn't be looked at.
    CardsUnread { game: String, why: String },
    /// A game's foils couldn't be looked at: which copy a foil is isn't
    /// known.
    FoilsUnread { game: String, why: String },
    /// Another device plays on the account, this game when Steam says and
    /// the library has it: farming waits until it stops.
    PlayedElsewhere { game: Option<String> },
    /// Another device took over playing: farming waits until it stops.
    TakenOver,
    /// The other device stopped: farming carries on in a while.
    ElsewhereStopped { carry_on_in: Duration },
    /// Something went wrong: farming tries again in a while, or, with no
    /// while, has stopped.
    WentWrong {
        trouble: Trouble,
        again_in: Option<Duration>,
    },
}
