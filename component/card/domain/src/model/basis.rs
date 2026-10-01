/// What a card is worth, as the user chooses to see it (research §1.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Basis {
    /// What a buyer pays: its lowest listing.
    #[default]
    List,
    /// What listing it at its lowest listing pays you, after Steam's fees.
    Net,
}
