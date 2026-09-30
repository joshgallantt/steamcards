/// What a card is worth, as the user chooses to see it (research §1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Basis {
    /// What a buyer pays: its lowest listing.
    #[default]
    List,
    /// What listing it at its lowest listing pays you, after Steam's fees.
    Net,
    /// What selling it now pays you: its best offer, after Steam's fees.
    /// Only a card's order book has offers.
    Instant,
}

impl Basis {
    /// The basis cards still to drop are valued on. A card that hasn't
    /// dropped can't be sold to an offer now, and an order book a card would
    /// cost a request each, so on the instant basis they're valued after
    /// fees instead, and the figures say so.
    pub fn still_to_drop(self) -> Basis {
        match self {
            Basis::Instant => Basis::Net,
            basis => basis,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instant_values_cards_still_to_drop_after_fees() {
        assert_eq!(Basis::default(), Basis::List);
        assert_eq!(Basis::Instant.still_to_drop(), Basis::Net);
        assert_eq!(Basis::Net.still_to_drop(), Basis::Net);
        assert_eq!(Basis::List.still_to_drop(), Basis::List);
    }
}
