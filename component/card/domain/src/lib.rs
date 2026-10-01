//! Card: the trading cards of the user's games, and what they're worth.
//! Each card comes in two kinds, normal and foil, and each kind makes a
//! badge of its own: so a game's set, of either kind, and how many of each
//! card the account has; the copies of cards the account holds, each an
//! item in its inventory; and their prices on the Steam market, in the
//! [`money`] component's amounts, with the [`account`]'s wallet. A card is
//! from the set of a game, so this component uses the [`game`] one.
//!
//! How Steam is asked (card pages, foils' pages, the inventory, the market)
//! is the data layer's business, behind [`CardRepository`] and
//! [`CardPriceRepository`], and so is how fast: every market lookup waits
//! its turn in Steam's one market queue. What cards are worth is the
//! valuation service's, worked out from the prices known. Farming and the
//! screens use the use cases and the service here, never the repositories.

mod model;
mod repository;
mod service;
mod use_cases;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{
    AssetId, Basis, Card, CardAsset, CardError, CardKind, CardSet, CardSets, Clock, Estimate,
    GameCards, Held, HeldCard, Lookup, MarketPause, NewItem, Price, PriceBook, PriceError,
    PriceEvent, PriceQuote, PricedCard, SetPrices, system_clock,
};
pub use repository::{CardPriceRepository, CardRepository};
pub use service::{expected_per_drop, held_value, on_completion, value_left, value_of};
pub use use_cases::{
    DefaultGetCardPricesUseCase, DefaultIdentifyCardsUseCase, DefaultKeepCardPricesUpToDateUseCase,
    DefaultLookAtCardsUseCase, DefaultLookAtFoilsUseCase, DefaultObserveNewItemsUseCase,
    DefaultRefreshCardPricesUseCase, DefaultSetCardsToPriceUseCase, GetCardPricesUseCase,
    IdentifyCardsUseCase, KeepCardPricesUpToDateUseCase, LookAtCardsUseCase, LookAtFoilsUseCase,
    ObserveNewItemsUseCase, RefreshCardPricesUseCase, SetCardsToPriceUseCase,
};
