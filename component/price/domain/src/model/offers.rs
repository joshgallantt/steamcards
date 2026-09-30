use chrono::{DateTime, Utc};

use crate::Price;

/// A card's order book as last looked up: what it said, and when. Nobody
/// buying or selling is an answer too, so an order book says when it was
/// looked up whatever it said.
#[derive(Debug, Clone, PartialEq)]
pub struct Offers {
    pub price: Price,
    pub looked_up_at: DateTime<Utc>,
}
