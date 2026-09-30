use std::sync::Arc;

use price::{
    GetPrices, GetWallet, PriceBook, PriceEvent, RefreshPrices, Wallet, WantPrices, WatchPrices,
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Keeps the cards' market prices coming: runs the market's price watcher,
/// tells it which games to price first, and asks again for a game whose card
/// just dropped.
pub struct Market {
    prices: GetPrices,
    want: WantPrices,
    watch: WatchPrices,
    refresh: RefreshPrices,
    wallet: GetWallet,
    tx: mpsc::Sender<PriceEvent>,
    events: mpsc::Receiver<PriceEvent>,
    watching: Option<CancellationToken>,
    /// The games last asked for, so the watcher hears only of a change.
    wanted: Vec<u32>,
}

impl Market {
    pub fn new(
        prices: GetPrices,
        want: WantPrices,
        watch: WatchPrices,
        refresh: RefreshPrices,
        wallet: GetWallet,
    ) -> Self {
        let (tx, events) = mpsc::channel(256);
        Self {
            prices,
            want,
            watch,
            refresh,
            wallet,
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
            drop((self.watch)(token.clone(), self.tx.clone()));
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
    pub fn want(&mut self, games: Vec<u32>) {
        if games != self.wanted {
            (self.want)(games.clone());
            self.wanted = games;
        }
    }

    /// One of the game's cards just dropped: its prices are looked at again,
    /// if they're over an hour old.
    pub fn dropped(&self, app_id: u32) {
        drop((self.refresh)(app_id));
    }

    pub fn try_recv(&mut self) -> Option<PriceEvent> {
        self.events.try_recv().ok()
    }

    /// Everything priced so far.
    pub fn book(&self) -> Arc<PriceBook> {
        (self.prices)()
    }

    /// The account's wallet: prices are shown in its currency. `None` until
    /// Steam has said.
    pub fn wallet(&self) -> Option<Wallet> {
        (self.wallet)()
    }
}

impl Drop for Market {
    fn drop(&mut self) {
        self.stop();
    }
}
