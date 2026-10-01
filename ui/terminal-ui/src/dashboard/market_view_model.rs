use std::sync::Arc;

use price::{
    GetPricesUseCase, GetWalletUseCase, KeepPricesUpToDateUseCase, PriceBook, PriceEvent,
    RefreshPricesUseCase, SetGamesToPriceUseCase, Wallet,
};
use steam_library::AppId;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Keeps the cards' market prices coming: runs the market's price watcher,
/// tells it which games to price first, and asks again for a game whose card
/// just dropped.
pub struct MarketViewModel {
    get_prices: Arc<dyn GetPricesUseCase>,
    set_games_to_price: Arc<dyn SetGamesToPriceUseCase>,
    keep_prices_up_to_date: Arc<dyn KeepPricesUpToDateUseCase>,
    refresh_prices: Arc<dyn RefreshPricesUseCase>,
    get_wallet: Arc<dyn GetWalletUseCase>,
    tx: mpsc::Sender<PriceEvent>,
    events: mpsc::Receiver<PriceEvent>,
    watching: Option<CancellationToken>,
    /// The games last asked for, so the watcher hears only of a change.
    wanted: Vec<AppId>,
}

impl MarketViewModel {
    pub fn new(
        get_prices: Arc<dyn GetPricesUseCase>,
        set_games_to_price: Arc<dyn SetGamesToPriceUseCase>,
        keep_prices_up_to_date: Arc<dyn KeepPricesUpToDateUseCase>,
        refresh_prices: Arc<dyn RefreshPricesUseCase>,
        get_wallet: Arc<dyn GetWalletUseCase>,
    ) -> Self {
        let (tx, events) = mpsc::channel(256);
        Self {
            get_prices,
            set_games_to_price,
            keep_prices_up_to_date,
            refresh_prices,
            get_wallet,
            tx,
            events,
            watching: None,
            wanted: Vec::new(),
        }
    }

    /// Starts pricing in the background, if it isn't already.
    pub fn start(&mut self) {
        if self.watching.is_none() {
            let token = CancellationToken::new();
            drop(
                self.keep_prices_up_to_date
                    .call(token.clone(), self.tx.clone()),
            );
            self.watching = Some(token);
        }
    }

    /// Stops pricing; what's priced so far is kept.
    pub fn stop(&mut self) {
        if let Some(token) = self.watching.take() {
            token.cancel();
        }
    }

    /// The games to price, most urgent first; the watcher hears only of a
    /// change.
    pub fn want(&mut self, games: Vec<AppId>) {
        if games != self.wanted {
            self.set_games_to_price.call(games.clone());
            self.wanted = games;
        }
    }

    /// One of the game's cards just dropped: its prices are looked at again,
    /// if they're over an hour old.
    pub fn dropped(&self, app_id: AppId) {
        drop(self.refresh_prices.call(app_id));
    }

    pub fn try_recv(&mut self) -> Option<PriceEvent> {
        self.events.try_recv().ok()
    }

    /// Everything priced so far.
    pub fn book(&self) -> Arc<PriceBook> {
        self.get_prices.call()
    }

    /// The account's wallet: prices are shown in its currency. `None` until
    /// Steam has said.
    pub fn wallet(&self) -> Option<Wallet> {
        self.get_wallet.call()
    }
}

impl Drop for MarketViewModel {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use price::test_support::{
        SpyKeepPricesUpToDateUseCase, SpyRefreshPricesUseCase, SpySetGamesToPriceUseCase,
        StubGetPricesUseCase, StubGetWalletUseCase,
    };

    use super::*;

    #[derive(Default)]
    struct Spies {
        told: Arc<SpySetGamesToPriceUseCase>,
        started: Arc<SpyKeepPricesUpToDateUseCase>,
        refreshed: Arc<SpyRefreshPricesUseCase>,
    }

    fn market(spies: &Spies) -> MarketViewModel {
        MarketViewModel::new(
            Arc::new(StubGetPricesUseCase::default()),
            spies.told.clone(),
            spies.started.clone(),
            spies.refreshed.clone(),
            Arc::new(StubGetWalletUseCase::new(None)),
        )
    }

    #[test]
    fn the_games_to_price_are_passed_on_only_when_they_change() {
        let spies = Spies::default();
        let mut m = market(&spies);
        m.want(vec![AppId(620)]);
        m.want(vec![AppId(620)]);
        m.want(vec![AppId(620), AppId(440)]);
        assert_eq!(
            spies.told.told(),
            [vec![AppId(620)], vec![AppId(620), AppId(440)]]
        );
    }

    #[tokio::test]
    async fn pricing_starts_once_until_stopped() {
        let spies = Spies::default();
        let mut m = market(&spies);
        m.start();
        m.start();
        assert_eq!(spies.started.starts(), 1);
        m.stop();
        m.start();
        assert_eq!(spies.started.starts(), 2);
    }

    #[tokio::test]
    async fn a_card_that_dropped_has_its_games_prices_looked_at_again() {
        let spies = Spies::default();
        market(&spies).dropped(AppId(960_910));
        assert_eq!(spies.refreshed.asked(), [AppId(960_910)]);
    }
}
