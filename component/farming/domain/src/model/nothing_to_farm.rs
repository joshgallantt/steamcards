/// Why there's nothing to farm.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NothingToFarm {
    /// The account has no games with trading cards.
    NoGames,
    /// Every card has dropped.
    AllDropped,
    /// "Only priority" is on, and the priority games are done.
    PrioritiesDone,
    /// Every game with cards left is skipped.
    AllSkipped,
    /// Steam isn't dropping cards for the games left: sale events' badges,
    /// and games set aside too often.
    NotDropping,
}
