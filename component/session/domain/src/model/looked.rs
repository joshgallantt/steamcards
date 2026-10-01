use card::CardSet;
use game::Game;

use crate::Found;

/// A look at one game, taken in.
pub struct Looked {
    /// The game as the farmer now sees it.
    pub game: Game,
    /// The drops it found since the farmer last saw the game.
    pub found: Option<Found>,
    /// The set it read: `None` when its page showed none, or was behind.
    pub(crate) set: Option<CardSet>,
}
