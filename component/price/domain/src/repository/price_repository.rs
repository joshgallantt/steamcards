use std::sync::Arc;

use async_trait::async_trait;

use crate::{Lookup, Offers, Price, PriceBook, PriceSettings, PricedCard, SetPrices, Wallet};

/// Where prices, the wallet and the market's settings come from, and where
/// they're kept. Declared here, beside the use cases that need it; the data
/// layer is written to fit.
///
/// Every lookup goes through Steam's one market queue, which keeps its own
/// pace: a lookup can take a while, and while Steam has paused lookups, it
/// says so at once, without asking. A market that couldn't be asked, or
/// didn't answer, is [`Lookup::Unanswered`]. An error is an answer that
/// couldn't be used, with a reason fit to show the user.
#[async_trait]
pub trait PriceRepository: Send + Sync {
    /// Everything priced so far, including what was kept from before a
    /// restart.
    fn book(&self) -> Arc<PriceBook>;

    /// Keeps a game's set in the book, in place of any before.
    fn keep_set(&self, set: SetPrices);

    /// Keeps a card's order book in the book, by its market hash name.
    fn keep_offers(&self, market_hash_name: &str, offers: Offers);

    /// A game's cards as the market lists them now, with their lowest
    /// listings: its normal cards, or its foils. The market's search doesn't
    /// say which currency it answers in, so its prices are read as the
    /// wallet's: until Steam has said what that is, the market isn't asked.
    async fn look_up_set(&self, app_id: u32, foil: bool)
    -> anyhow::Result<Lookup<Vec<PricedCard>>>;

    /// A card's order book now, by its market hash name: its lowest listing
    /// and its best offer.
    async fn look_up_offers(&self, market_hash_name: &str) -> anyhow::Result<Lookup<Price>>;

    /// The account's wallet, as Steam last said; `None` until it has.
    fn wallet(&self) -> Option<Wallet>;

    fn settings(&self) -> PriceSettings;

    /// Errs when the settings couldn't be kept, so a caller can't report a
    /// change that didn't happen. What `settings` returns afterwards is what
    /// was kept.
    fn save_settings(&self, settings: PriceSettings) -> anyhow::Result<()>;

    /// The games wanted priced, most urgent first, as last said.
    fn wanted(&self) -> Vec<u32>;

    fn want(&self, app_ids: Vec<u32>);
}
