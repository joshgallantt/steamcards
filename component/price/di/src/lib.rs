//! Where the market domain meets its data layer: one repository over the
//! Steam session, the config file and the price cache, handed to every use
//! case, on the system's clock. The composition root names the session and
//! the files.

use std::sync::Arc;

use config_file::{ConfigFile, PriceCache};
use price::{
    Clock, GetPriceSettings, GetPrices, GetWallet, PriceOffers, PriceRepository, RefreshPrices,
    SetBasis, WantPrices, WatchPrices, get_price_settings, get_prices, get_wallet, price_offers,
    refresh_prices, set_basis, system_clock, want_prices, watch_prices,
};
use price_data::SteamPriceRepository;
use steam_api::Session;

pub struct PriceComponent {
    pub prices: GetPrices,
    pub want: WantPrices,
    pub watch: WatchPrices,
    pub refresh: RefreshPrices,
    pub offers: PriceOffers,
    pub wallet: GetWallet,
    pub settings: GetPriceSettings,
    pub set_basis: SetBasis,
}

impl PriceComponent {
    pub fn new(session: Arc<Session>, file: Arc<ConfigFile>, prices: Arc<PriceCache>) -> Self {
        Self::over(
            Arc::new(SteamPriceRepository::new(session, file, prices)),
            system_clock(),
        )
    }

    pub fn over(repo: Arc<dyn PriceRepository>, clock: Clock) -> Self {
        Self {
            prices: get_prices(repo.clone()),
            want: want_prices(repo.clone()),
            watch: watch_prices(repo.clone(), clock.clone()),
            refresh: refresh_prices(repo.clone(), clock.clone()),
            offers: price_offers(repo.clone(), clock),
            wallet: get_wallet(repo.clone()),
            settings: get_price_settings(repo.clone()),
            set_basis: set_basis(repo),
        }
    }
}
