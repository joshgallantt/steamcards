use std::sync::Arc;

use async_trait::async_trait;

use crate::{Lookup, MarketSettings, Price, PriceBook, PricedCard, SetPrices, Wallet};

/// Where prices, the wallet and the market's settings come from, and where
/// they're kept. Declared here, beside the use cases that need it; the data
/// layer is written to fit.
///
/// Every lookup goes through Steam's one market queue, which keeps its own
/// pace: a lookup can take a while, and while Steam has paused lookups, it
/// says so at once, without asking. Errors carry a reason fit to show the
/// user.
#[async_trait]
pub trait MarketRepository: Send + Sync {
    /// Everything priced so far, including what was kept from before a
    /// restart.
    fn book(&self) -> Arc<PriceBook>;

    /// Keeps a game's set in the book, in place of any before.
    fn keep_set(&self, set: SetPrices);

    /// Keeps a card's order book in the book, by its market hash name.
    fn keep_offers(&self, market_hash_name: &str, price: Price);

    /// A game's cards as the market lists them now, with their lowest
    /// listings: its normal cards, or its foils.
    async fn look_up_set(&self, app_id: u32, foil: bool)
    -> anyhow::Result<Lookup<Vec<PricedCard>>>;

    /// A card's order book now, by its market hash name: its lowest listing
    /// and its best offer.
    async fn look_up_offers(&self, market_hash_name: &str) -> anyhow::Result<Lookup<Price>>;

    /// The account's wallet, as Steam last said; `None` until it has.
    fn wallet(&self) -> Option<Wallet>;

    fn settings(&self) -> MarketSettings;

    /// Errs when the settings couldn't be kept, so a caller can't report a
    /// change that didn't happen. What `settings` returns afterwards is what
    /// was kept.
    fn save_settings(&self, settings: MarketSettings) -> anyhow::Result<()>;

    /// The games wanted priced, most urgent first, as last said.
    fn wanted(&self) -> Vec<u32>;

    fn want(&self, app_ids: Vec<u32>);
}
