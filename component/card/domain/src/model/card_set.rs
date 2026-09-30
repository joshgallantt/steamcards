use crate::Card;

/// A game's set of trading cards, or its foils, and how many of each the
/// account has, in the set's order. Empty while it isn't known: a set is
/// read from the game's own card page.
///
/// It's a measure of its own beside the game's drops: the same card can drop
/// twice, so 3 of 4 drops can be 2 of 5 cards and a spare.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardSet {
    cards: Vec<Card>,
}

impl CardSet {
    pub fn new(cards: Vec<Card>) -> Self {
        Self { cards }
    }

    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether it's unknown: no set has an empty page.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// How many of `name` the account has; none when it isn't in the set.
    pub fn owned(&self, name: &str) -> u32 {
        self.cards
            .iter()
            .find(|c| c.name == name)
            .map_or(0, |c| c.owned)
    }

    /// How many cards of the set the account has at least one of.
    pub fn collected(&self) -> usize {
        self.cards.iter().filter(|c| c.owned > 0).count()
    }

    /// Whether the account has every card of the set, so a badge can be
    /// crafted. `false` while the set isn't known.
    pub fn is_full(&self) -> bool {
        !self.cards.is_empty() && self.cards.iter().all(|c| c.owned > 0)
    }

    /// Copies held beyond one of each card of the set: a badge level takes
    /// one of each.
    pub fn spares(&self) -> u32 {
        self.cards.iter().map(Card::spares).sum()
    }

    /// The cards of the set the account has none of: what a badge is short
    /// of. None while the set isn't known.
    pub fn missing(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter().filter(|c| c.owned == 0)
    }

    /// Counts one more copy of `name` in: whether the set has it.
    pub fn count_in(&mut self, name: &str) -> bool {
        match self.cards.iter_mut().find(|c| c.name == name) {
            Some(card) => {
                card.owned += 1;
                true
            }
            None => false,
        }
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
    fn a_set_knows_what_the_account_has_of_it() {
        let mut portal = CardSet::default();
        assert!(!portal.is_full(), "the set isn't known yet");
        assert_eq!(portal.spares(), 0, "the set isn't known yet");
        assert_eq!(portal.missing().count(), 0, "nor what it's short of");

        portal = CardSet::new(vec![
            card("Atlas", 2),
            card("P-Body", 0),
            card("Wheatley", 1),
        ]);
        assert_eq!(portal.collected(), 2);
        assert!(!portal.is_full());
        assert!(portal.count_in("P-Body"));
        assert!(portal.is_full());
        assert!(!portal.count_in("GLaDOS"), "no such card in the set");
    }

    #[test]
    fn a_set_counts_its_spares_and_what_it_is_short_of() {
        // Heavy Rain: three drops so far, one of them a second Madison.
        let heavy_rain = CardSet::new(vec![
            card("Ethan", 0),
            card("Carter", 0),
            card("Madison", 2),
            card("Norman", 0),
            card("Scott", 1),
        ]);

        assert_eq!(
            (heavy_rain.collected(), heavy_rain.len()),
            (2, 5),
            "2 of 5 cards"
        );
        assert_eq!(heavy_rain.spares(), 1);
        assert_eq!(heavy_rain.owned("Madison"), 2);
        assert_eq!(heavy_rain.owned("Anarchist"), 0, "not in the set");
        let missing: Vec<&str> = heavy_rain.missing().map(|c| c.name.as_str()).collect();
        assert_eq!(missing, ["Ethan", "Carter", "Norman"]);
        assert!(!heavy_rain.is_full());
    }
}
