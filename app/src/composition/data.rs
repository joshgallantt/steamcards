use std::{path::PathBuf, sync::Arc};

use config_file::{ConfigFile, ConfigLock, CredentialStore};
use session::SessionKeeper;
use steam_api::SteamClient;

use crate::settings::Settings;

/// Phase one: the files and the Steam client everything else runs on. One
/// client, shared, so when Steam rejects the sign-in every screen knows.
/// Where the market's prices are kept: a file of their own beside the config
/// file, which the card component opens. And the keeper of this session of
/// farming, which holds it in memory alone.
pub(crate) struct DataAssembler {
    /// This steamcards' hold on its config file, taken before the file is
    /// read: another can't use it until this one exits.
    pub lock: ConfigLock,
    pub config: Arc<ConfigFile>,
    pub prices: PathBuf,
    pub steam: Arc<SteamClient>,
    pub sessions: Arc<SessionKeeper>,
}

impl DataAssembler {
    pub(crate) fn new(settings: &Settings) -> anyhow::Result<Self> {
        let Some(lock) = ConfigLock::take(&settings.config_path)? else {
            anyhow::bail!(
                "steamcards is already running with {}. Quit that one first, or give this one a \
                 config file of its own with STEAMCARDS_CONFIG.",
                settings.config_path.display()
            );
        };
        let config = Arc::new(ConfigFile::open(settings.config_path.clone())?);
        let credentials: Arc<dyn CredentialStore> = config.clone();
        Ok(Self {
            lock,
            prices: settings.prices_path.clone(),
            steam: Arc::new(SteamClient::new(credentials, &settings.debug_log)),
            sessions: Arc::default(),
            config,
        })
    }
}
