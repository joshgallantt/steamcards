use crate::Basis;

/// The market's settings, kept apart from the preferences so they never
/// hold a market type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PriceSettings {
    /// How money is shown: list prices, unless the user chooses otherwise.
    pub basis: Basis,
}
