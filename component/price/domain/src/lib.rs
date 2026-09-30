//! Price: what the user's cards are worth on the Steam market, in the
//! [`money`] component's amounts. The prices of each game's set and each
//! card's best offers, the wallet's currency and fees, the value basis the
//! user picks, and Steam's pause on market lookups.
//!
//! It values the [`card`] component's cards, the copies held and a game's
//! set, and the drops the [`game`] component's games still have to come. It
//! knows nothing of farming: whoever shows a session's cards hands them over
//! as [`HeldCard`]s. How Steam is asked is the data layer's business, behind
//! [`PriceRepository`], and so is how fast: every lookup waits its turn in
//! Steam's one market queue. This component decides what to price and in
//! what order. The numbers it runs on, and where each comes from, are in
//! `rules.rs`.

mod clock;
mod model;
mod pricing;
mod repository;
mod rules;
mod use_cases;
mod valuation;
mod watcher;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use clock::{Clock, system_clock};
pub use model::{
    Basis, Estimate, Held, HeldCard, Lookup, MarketPause, Offers, Price, PriceBook, PriceError,
    PriceEvent, PriceEventKind, PriceQuote, PriceSettings, PricedCard, QuoteSource, SetPrices,
    Wallet,
};
pub use repository::PriceRepository;
pub use use_cases::{
    DefaultGetPriceSettingsUseCase, DefaultGetPricesUseCase, DefaultGetWalletUseCase,
    DefaultKeepPricesUpToDateUseCase, DefaultLookUpOffersUseCase, DefaultRefreshPricesUseCase,
    DefaultSetBasisUseCase, DefaultSetGamesToPriceUseCase, GetPriceSettingsUseCase,
    GetPricesUseCase, GetWalletUseCase, KeepPricesUpToDateUseCase, LookUpOffersUseCase,
    RefreshPricesUseCase, SetBasisUseCase, SetGamesToPriceUseCase,
};
pub use valuation::{expected_per_drop, held_value, on_completion, value_left, value_of};
