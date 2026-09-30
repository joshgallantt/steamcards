use card::CardAsset;

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
    /// Neither Steam nor the card page could tell: the page's counts went up
    /// for no card, or for more than these drops.
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
