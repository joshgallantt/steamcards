//! Where the price domain meets its data layer: one repository over the
//! Steam session, the config file and the price cache, handed to every use
//! case, on the system's clock. The composition root names the Steam client and
//! the files.

use std::sync::Arc;

use config_file::{ConfigFile, PriceCache};
use price::{
    Clock, DefaultGetPriceSettingsUseCase, DefaultGetPricesUseCase, DefaultGetWalletUseCase,
    DefaultKeepPricesUpToDateUseCase, DefaultLookUpOffersUseCase, DefaultRefreshPricesUseCase,
    DefaultSetBasisUseCase, DefaultSetGamesToPriceUseCase, GetPriceSettingsUseCase,
    GetPricesUseCase, GetWalletUseCase, KeepPricesUpToDateUseCase, LookUpOffersUseCase,
    PriceRepository, RefreshPricesUseCase, SetBasisUseCase, SetGamesToPriceUseCase, system_clock,
};
use price_data::SteamPriceRepository;
use steam_api::SteamClient;

pub struct PriceComponent {
    pub get_prices: Arc<dyn GetPricesUseCase>,
    pub set_games_to_price: Arc<dyn SetGamesToPriceUseCase>,
    pub keep_prices_up_to_date: Arc<dyn KeepPricesUpToDateUseCase>,
    pub refresh_prices: Arc<dyn RefreshPricesUseCase>,
    pub look_up_offers: Arc<dyn LookUpOffersUseCase>,
    pub get_wallet: Arc<dyn GetWalletUseCase>,
    pub get_price_settings: Arc<dyn GetPriceSettingsUseCase>,
    pub set_basis: Arc<dyn SetBasisUseCase>,
}

impl PriceComponent {
    pub fn new(steam: Arc<SteamClient>, file: Arc<ConfigFile>, prices: Arc<PriceCache>) -> Self {
        Self::over(
            Arc::new(SteamPriceRepository::new(steam, file, prices)),
            system_clock(),
        )
    }

    pub fn over(repo: Arc<dyn PriceRepository>, clock: Clock) -> Self {
        Self {
            get_prices: Arc::new(DefaultGetPricesUseCase::new(repo.clone())),
            set_games_to_price: Arc::new(DefaultSetGamesToPriceUseCase::new(repo.clone())),
            keep_prices_up_to_date: Arc::new(DefaultKeepPricesUpToDateUseCase::new(
                repo.clone(),
                clock.clone(),
            )),
            refresh_prices: Arc::new(DefaultRefreshPricesUseCase::new(
                repo.clone(),
                clock.clone(),
            )),
            look_up_offers: Arc::new(DefaultLookUpOffersUseCase::new(repo.clone(), clock)),
            get_wallet: Arc::new(DefaultGetWalletUseCase::new(repo.clone())),
            get_price_settings: Arc::new(DefaultGetPriceSettingsUseCase::new(repo.clone())),
            set_basis: Arc::new(DefaultSetBasisUseCase::new(repo)),
        }
    }
}
