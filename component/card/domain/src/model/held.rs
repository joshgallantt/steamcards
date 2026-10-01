use std::time::Duration;

use money::Money;

/// What cards held are worth: at least `total`, since cards that aren't
/// priced can only add to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Held {
    pub total: Money,
    /// Cards counted in the total.
    pub priced: u32,
    /// Cards that can be sold but aren't counted: not priced yet, not
    /// known yet, nobody selling, a failed lookup, or priced in another
    /// currency.
    pub unpriced: u32,
    /// Cards that can't be sold, left out.
    pub not_marketable: u32,
    /// How old the oldest price over 6 hours old in the total is; `None`
    /// while every price counted is fresh.
    pub oldest: Option<Duration>,
}
