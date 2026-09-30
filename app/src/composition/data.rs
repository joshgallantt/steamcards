use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore, PriceCache};
use keep_awake::KeepAwake;
use steam_api::SteamClient;

use crate::settings::Settings;

/// Phase one: the files and the Steam session everything else runs on. One
/// session, shared, so when Steam rejects the sign-in every screen knows. The
/// market's prices, kept in a file of their own beside the config file. And
/// the system's way of staying awake while games play.
pub(crate) struct DataAssembler {
    pub config: Arc<ConfigFile>,
    pub prices: Arc<PriceCache>,
    pub steam: Arc<SteamClient>,
    pub awake: Arc<KeepAwake>,
}

impl DataAssembler {
    pub(crate) fn new(settings: &Settings) -> anyhow::Result<Self> {
        let config = Arc::new(ConfigFile::open(settings.config_path.clone())?);
        let credentials: Arc<dyn CredentialStore> = config.clone();
        Ok(Self {
            prices: Arc::new(PriceCache::open(settings.prices_path.clone())),
            steam: Arc::new(SteamClient::new(credentials, &settings.debug_log)),
            awake: Arc::new(KeepAwake::system(&settings.debug_log)),
            config,
        })
    }
}
