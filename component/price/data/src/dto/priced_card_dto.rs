use price::PricedCard;
use serde::{Deserialize, Serialize};

use crate::dto::PriceDto;

/// A card of a set, and its price, as kept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PricedCardDto {
    name: String,
    market_hash_name: String,
    price: PriceDto,
}

impl PricedCardDto {
    pub(crate) fn new(card: &PricedCard) -> Self {
        Self {
            name: card.name.clone(),
            market_hash_name: card.market_hash_name.clone(),
            price: PriceDto::new(&card.price),
        }
    }

    /// `None` when its price doesn't read.
    pub(crate) fn into_domain(self) -> Option<PricedCard> {
        Some(PricedCard {
            price: self.price.to_domain()?,
            name: self.name,
            market_hash_name: self.market_hash_name,
        })
    }
}
