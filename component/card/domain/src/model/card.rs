/// A trading card from a game's set. Its identity is its name, within the
/// set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub name: String,
    /// How many the account has. A card can drop more than once, so this is
    /// a count, not a tick.
    pub owned: u32,
}

impl Card {
    /// Copies beyond the one a badge level takes.
    pub fn spares(&self) -> u32 {
        self.owned.saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(name: &str, owned: u32) -> Card {
        Card {
            name: name.into(),
            owned,
        }
    }

    #[test]
    fn copies_beyond_one_are_spares() {
        assert_eq!(card("Madison", 0).spares(), 0);
        assert_eq!(card("Madison", 1).spares(), 0);
        assert_eq!(card("Madison", 3).spares(), 2);
    }
}
