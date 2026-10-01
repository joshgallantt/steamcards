//! Where the card domain meets its data layer: the cards as Steam shows them
//! through one repository, and their prices looked up through Steam's market
//! and kept in the config file and a file of their own through another,
//! each handed to the use cases that need it, on the system's clock. The
//! composition root names the Steam client and the files.

use std::{path::PathBuf, sync::Arc};

use card::{
    CardPriceRepository, CardRepository, Clock, DefaultGetCardPricesUseCase,
    DefaultIdentifyCardsUseCase, DefaultKeepCardPricesUpToDateUseCase, DefaultLookAtCardsUseCase,
    DefaultLookAtFoilsUseCase, DefaultObserveNewItemsUseCase, DefaultRefreshCardPricesUseCase,
    DefaultSetCardsToPriceUseCase, GetCardPricesUseCase, IdentifyCardsUseCase,
    KeepCardPricesUpToDateUseCase, LookAtCardsUseCase, LookAtFoilsUseCase, ObserveNewItemsUseCase,
    RefreshCardPricesUseCase, SetCardsToPriceUseCase, system_clock,
};
use card_data::{
    CardClient, DefaultCardPriceRepository, DefaultCardRepository, FilePriceStore, MarketClient,
    PriceStore, SteamCardClient, SteamMarketClient,
};
use config_file::ConfigFile;
use debug_log::DebugLog;
use steam_api::SteamClient;

pub struct CardComponent {
    pub look_at_cards: Arc<dyn LookAtCardsUseCase>,
    pub look_at_foils: Arc<dyn LookAtFoilsUseCase>,
    pub identify_cards: Arc<dyn IdentifyCardsUseCase>,
    pub observe_new_items: Arc<dyn ObserveNewItemsUseCase>,
    pub get_card_prices: Arc<dyn GetCardPricesUseCase>,
    pub set_cards_to_price: Arc<dyn SetCardsToPriceUseCase>,
    pub keep_card_prices_up_to_date: Arc<dyn KeepCardPricesUpToDateUseCase>,
    pub refresh_card_prices: Arc<dyn RefreshCardPricesUseCase>,
}

impl CardComponent {
    /// Keeps the prices in the file at `prices`, and Steam's pause on the
    /// market in the config file.
    pub fn new(steam: Arc<SteamClient>, file: Arc<ConfigFile>, prices: PathBuf) -> Self {
        let log = steam.log().clone();
        Self::over(
            Arc::new(SteamCardClient::new(steam.clone())),
            Arc::new(SteamMarketClient::new(steam)),
            Arc::new(FilePriceStore::open(file, prices)),
            log,
            system_clock(),
        )
    }

    /// Over clients and a store of its own: the repositories are built here,
    /// and never let out.
    pub fn over(
        cards: Arc<dyn CardClient>,
        market: Arc<dyn MarketClient>,
        store: Arc<dyn PriceStore>,
        log: DebugLog,
        clock: Clock,
    ) -> Self {
        let repo: Arc<dyn CardRepository> = Arc::new(DefaultCardRepository::new(cards));
        let prices: Arc<dyn CardPriceRepository> =
            Arc::new(DefaultCardPriceRepository::new(market, store, log));
        Self {
            look_at_cards: Arc::new(DefaultLookAtCardsUseCase::new(repo.clone())),
            look_at_foils: Arc::new(DefaultLookAtFoilsUseCase::new(repo.clone())),
            identify_cards: Arc::new(DefaultIdentifyCardsUseCase::new(repo.clone())),
            observe_new_items: Arc::new(DefaultObserveNewItemsUseCase::new(repo)),
            get_card_prices: Arc::new(DefaultGetCardPricesUseCase::new(prices.clone())),
            set_cards_to_price: Arc::new(DefaultSetCardsToPriceUseCase::new(prices.clone())),
            keep_card_prices_up_to_date: Arc::new(DefaultKeepCardPricesUpToDateUseCase::new(
                prices.clone(),
                clock.clone(),
            )),
            refresh_card_prices: Arc::new(DefaultRefreshCardPricesUseCase::new(prices, clock)),
        }
    }
}
