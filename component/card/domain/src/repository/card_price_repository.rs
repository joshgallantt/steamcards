use std::sync::Arc;

use async_trait::async_trait;
use game::AppId;

use crate::{CardKind, Lookup, PriceBook, PricedCard, SetPrices};

/// Where cards' prices come from, and where they're kept. Declared here,
/// beside the use cases that need it; the data layer is written to fit.
///
/// Every lookup goes through Steam's one market queue, which keeps its own
/// pace: a lookup can take a while, and while Steam has paused lookups, it
/// says so at once, without asking. A market that couldn't be asked, or
/// didn't answer, is [`Lookup::Unanswered`]. An error is an answer that
/// couldn't be used, with a reason fit to show the user.
#[async_trait]
pub trait CardPriceRepository: Send + Sync {
    /// Everything priced so far, including what was kept from before a
    /// restart.
    fn book(&self) -> Arc<PriceBook>;

    /// Keeps a game's set in the book, in place of any before.
    fn keep_set(&self, set: SetPrices);

    /// A game's cards of one kind as the market lists them now, with their
    /// lowest listings: its normal cards, or its foils. The market's search
    /// doesn't say which currency it answers in, so its prices are read as
    /// the wallet's: until Steam has said what that is, the market isn't
    /// asked.
    async fn look_up_set(
        &self,
        app_id: AppId,
        kind: CardKind,
    ) -> anyhow::Result<Lookup<Vec<PricedCard>>>;

    /// The games wanted priced, most urgent first, as last said.
    fn wanted(&self) -> Vec<AppId>;

    fn want(&self, app_ids: Vec<AppId>);
}
