//! Where the price domain meets its data layer: prices looked up through the
//! Steam session and kept in the config file and a file of their own, through
//! one repository handed to every use case, on the system's clock. The
//! composition root names the Steam client and the files.

use std::{path::PathBuf, sync::Arc};

use config_file::ConfigFile;
use debug_log::DebugLog;
use price::{
    Clock, DefaultGetPriceSettingsUseCase, DefaultGetPricesUseCase, DefaultGetWalletUseCase,
    DefaultKeepPricesUpToDateUseCase, DefaultLookUpOffersUseCase, DefaultRefreshPricesUseCase,
    DefaultSetBasisUseCase, DefaultSetGamesToPriceUseCase, GetPriceSettingsUseCase,
    GetPricesUseCase, GetWalletUseCase, KeepPricesUpToDateUseCase, LookUpOffersUseCase,
    PriceRepository, RefreshPricesUseCase, SetBasisUseCase, SetGamesToPriceUseCase, system_clock,
};
use price_data::{
    DefaultPriceRepository, FilePriceStore, MarketClient, PriceStore, SteamMarketClient,
};
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
    /// Keeps the prices in the file at `prices`.
    pub fn new(steam: Arc<SteamClient>, file: Arc<ConfigFile>, prices: PathBuf) -> Self {
        let log = steam.log().clone();
        Self::over(
            Arc::new(SteamMarketClient::new(steam)),
            Arc::new(FilePriceStore::open(file, prices)),
            log,
            system_clock(),
        )
    }

    /// Over a client and a store of its own: the repository is built here,
    /// and never let out.
    pub fn over(
        client: Arc<dyn MarketClient>,
        store: Arc<dyn PriceStore>,
        log: DebugLog,
        clock: Clock,
    ) -> Self {
        let repo: Arc<dyn PriceRepository> =
            Arc::new(DefaultPriceRepository::new(client, store, log));
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
