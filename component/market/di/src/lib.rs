//! Where the market domain meets its data layer: one repository over the
//! Steam session, the config file and the price cache, handed to every use
//! case, on the system's clock. The composition root names the session and
//! the files.

use std::sync::Arc;

use config_file::{ConfigFile, PriceCache};
use market::{
    Clock, GetMarketSettings, GetPrices, GetWallet, MarketRepository, PriceOffers, RefreshPrices,
    SetBasis, WantPrices, WatchPrices, get_market_settings, get_prices, get_wallet, price_offers,
    refresh_prices, set_basis, system_clock, want_prices, watch_prices,
};
use market_data::SteamMarketRepository;
use steam_api::Session;

pub struct MarketComponent {
    pub prices: GetPrices,
    pub want: WantPrices,
    pub watch: WatchPrices,
    pub refresh: RefreshPrices,
    pub offers: PriceOffers,
    pub wallet: GetWallet,
    pub settings: GetMarketSettings,
    pub set_basis: SetBasis,
}

impl MarketComponent {
    pub fn new(session: Arc<Session>, file: Arc<ConfigFile>, prices: Arc<PriceCache>) -> Self {
        Self::over(
            Arc::new(SteamMarketRepository::new(session, file, prices)),
            system_clock(),
        )
    }

    pub fn over(repo: Arc<dyn MarketRepository>, clock: Clock) -> Self {
        Self {
            prices: get_prices(repo.clone()),
            want: want_prices(repo.clone()),
            watch: watch_prices(repo.clone(), clock.clone()),
            refresh: refresh_prices(repo.clone(), clock.clone()),
            offers: price_offers(repo.clone(), clock),
            wallet: get_wallet(repo.clone()),
            settings: get_market_settings(repo.clone()),
            set_basis: set_basis(repo),
        }
    }
}
