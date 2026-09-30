//! Market: what the user's cards are worth on the Steam market, in the
//! [`money`] component's amounts. The prices of each game's set and each
//! card's best offers, the wallet's currency and fees, the value basis the
//! user picks, and Steam's pause on lookups.
//!
//! It values the [`library`] component's cards: the copies held, a game's
//! set, the drops still to come. It knows nothing of farming: whoever shows
//! a session's cards hands them over as [`HeldCard`]s. How Steam is asked is
//! the data layer's business, behind [`MarketRepository`], and so is how
//! fast: every lookup waits its turn in Steam's one market queue. The market
//! decides what to price and in what order. The numbers it runs on, and
//! where each comes from, are in `rules.rs`.

mod model;
mod repository;
mod rules;
mod use_cases;
mod valuation;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use model::{
    Basis, Estimate, Held, HeldCard, Lookup, MarketError, MarketEvent, MarketEventKind,
    MarketPause, MarketSettings, Offers, Price, PriceBook, PriceQuote, PricedCard, QuoteSource,
    SetPrices, Wallet,
};
pub use repository::MarketRepository;
pub use use_cases::{
    Clock, GetMarketSettings, GetPrices, GetWallet, PriceOffers, RefreshPrices, SetBasis,
    WantPrices, WatchPrices, get_market_settings, get_prices, get_wallet, price_offers,
    refresh_prices, set_basis, system_clock, want_prices, watch_prices,
};
pub use valuation::{expected_per_drop, held_value, on_completion, value_left, value_of};
