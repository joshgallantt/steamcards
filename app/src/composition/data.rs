use std::{path::PathBuf, sync::Arc};

use config_file::{ConfigFile, CredentialStore};
use keep_awake::KeepAwake;
use session::SessionKeeper;
use steam_api::SteamClient;

use crate::settings::Settings;

/// Phase one: the files and the Steam client everything else runs on. One
/// client, shared, so when Steam rejects the sign-in every screen knows.
/// Where the market's prices are kept: a file of their own beside the config
/// file, which the price component opens. The
/// keeper of this session of farming, which holds it in memory alone. And
/// the system's way of staying awake while games play.
pub(crate) struct DataAssembler {
    pub config: Arc<ConfigFile>,
    pub prices: PathBuf,
    pub steam: Arc<SteamClient>,
    pub awake: Arc<KeepAwake>,
    pub sessions: Arc<SessionKeeper>,
}

impl DataAssembler {
    pub(crate) fn new(settings: &Settings) -> anyhow::Result<Self> {
        let config = Arc::new(ConfigFile::open(settings.config_path.clone())?);
        let credentials: Arc<dyn CredentialStore> = config.clone();
        Ok(Self {
            prices: settings.prices_path.clone(),
            steam: Arc::new(SteamClient::new(credentials, &settings.debug_log)),
            awake: Arc::new(KeepAwake::system(&settings.debug_log)),
            sessions: Arc::default(),
            config,
        })
    }
}
