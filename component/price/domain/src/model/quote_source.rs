/// Where a quote came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuoteSource {
    /// A game's set, priced a page of cards at a time: lowest listings only.
    Search,
    /// One card's order book: its lowest listing and its best offer.
    OrderBook,
}
