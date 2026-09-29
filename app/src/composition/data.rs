use std::sync::Arc;

use config_file::{ConfigFile, CredentialStore};
use keep_awake::KeepAwake;
use steam_api::Session;

use crate::settings::Settings;

/// Phase one: the file and the Steam session everything else runs on. One
/// session, shared, so when Steam rejects the sign-in every screen knows. And
/// the system's way of staying awake while games play.
pub(crate) struct DataAssembler {
    pub config: Arc<ConfigFile>,
    pub steam: Arc<Session>,
    pub awake: Arc<KeepAwake>,
}

impl DataAssembler {
    pub(crate) fn new(settings: &Settings) -> anyhow::Result<Self> {
        let config = Arc::new(ConfigFile::open(settings.config_path.clone())?);
        let credentials: Arc<dyn CredentialStore> = config.clone();
        Ok(Self {
            steam: Arc::new(Session::new(credentials, &settings.debug_log)),
            awake: Arc::new(KeepAwake::system(&settings.debug_log)),
            config,
        })
    }
}
