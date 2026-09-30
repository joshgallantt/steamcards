use crate::MarketPause;

/// What the market said to a lookup, that Steam has paused lookups, or
/// that the market couldn't be asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Lookup<T> {
    Found(T),
    Paused(MarketPause),
    /// The market couldn't be asked, or didn't answer, for this reason: not
    /// signed in, no network, a server error twice, or the wallet's currency
    /// not known yet to read prices in. Nothing is wrong with the prices,
    /// so none is taken as failed: it's asked again soon.
    Unanswered(String),
}
