use std::sync::Arc;

use config_file::ConfigFile;
use preferences::Preferences;

use crate::dto::PreferencesDto;

/// Where the preferences are kept.
pub trait PreferencesStore: Send + Sync {
    /// The preferences kept: the defaults, when none are, or when they don't
    /// read.
    fn preferences(&self) -> Preferences;

    /// Errs when they couldn't be kept, so nothing reports a change that
    /// didn't happen.
    fn save(&self, preferences: &Preferences) -> anyhow::Result<()>;
}

/// The preferences, in the config file beside its other fields.
pub struct FilePreferencesStore {
    file: Arc<ConfigFile>,
}

impl FilePreferencesStore {
    pub fn new(file: Arc<ConfigFile>) -> Self {
        Self { file }
    }
}

impl PreferencesStore for FilePreferencesStore {
    fn preferences(&self) -> Preferences {
        self.file
            .read::<PreferencesDto>()
            .unwrap_or_default()
            .into_domain()
    }

    fn save(&self, preferences: &Preferences) -> anyhow::Result<()> {
        self.file.write(&PreferencesDto::new(preferences))
    }
}
