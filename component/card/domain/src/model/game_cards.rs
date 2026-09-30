use game::Game;

use crate::CardSet;

/// A game as its own card page shows it: the game, with its drops and hours
/// as they stand, and its set, with how many of each card the account has.
/// The set is empty when the page shows none.
#[derive(Debug, Clone, PartialEq)]
pub struct GameCards {
    pub game: Game,
    pub set: CardSet,
}
