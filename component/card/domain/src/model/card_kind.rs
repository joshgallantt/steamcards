/// What kind of trading card a copy is: a normal card, or a foil, a rarer
/// copy of the same card with a shiny border. Steam calls it the card's
/// border, and keeps the kinds apart: each makes a badge of its own, so a
/// game's set comes in both, counted apart, and each kind of a card is an
/// item of its own on the market.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CardKind {
    #[default]
    Normal,
    Foil,
}
