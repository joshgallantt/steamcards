use money::Money;

/// A value that's an estimate: what cards still to drop are likely worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estimate {
    pub value: Money,
    /// Foils are left out: how often one drops isn't known.
    pub excl_foils: bool,
    /// Games with drops left whose cards aren't priced, so aren't in it.
    pub unpriced_games: u32,
}
