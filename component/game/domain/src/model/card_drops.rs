/// A game's card drops. Playing a game drops about half its set; which cards
/// drop is up to Steam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CardDrops {
    /// Cards dropped so far.
    pub received: u32,
    /// Cards still to drop.
    pub remaining: u32,
}

impl CardDrops {
    /// Every card playing the game drops, dropped or not.
    pub fn total(&self) -> u32 {
        self.received + self.remaining
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_games_drops_are_those_received_and_those_to_come() {
        let drops = CardDrops {
            received: 2,
            remaining: 2,
        };
        assert_eq!(drops.total(), 4);
    }
}
