//! Someone arranging what gets farmed first with steamcards: the preferences
//! component wired as the composition root wires it, over the real data
//! layer and a real config file in a folder of its own.

use std::{fs, path::PathBuf, sync::Arc};

use config_file::ConfigFile;
use preferences::{Preferences, PreferencesError, Tier};
use preferences_di::PreferencesComponent;
use steam_library::AppId;

pub(crate) struct Player {
    config: PathBuf,
    preferences: PreferencesComponent,
}

impl Player {
    /// Someone who hasn't chosen anything yet.
    pub(crate) fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "steamcards-preferences-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        let config = dir.join("config.json");
        let preferences = component(&config);
        Self {
            config,
            preferences,
        }
    }

    pub(crate) fn prefs(&self) -> Preferences {
        self.preferences.get_preferences.call()
    }

    /// The games farmed first, in order.
    pub(crate) fn priorities(&self) -> Vec<u32> {
        self.prefs().priority_games.iter().map(|g| g.0).collect()
    }

    /// The games never farmed.
    pub(crate) fn skipped(&self) -> Vec<u32> {
        self.prefs().skipped_games.iter().map(|g| g.0).collect()
    }

    pub(crate) fn ranks(&self, app_id: u32, rank: usize) {
        self.sets(app_id, Tier::Priority(rank)).unwrap();
    }

    pub(crate) fn sets(&self, app_id: u32, tier: Tier) -> Result<(), PreferencesError> {
        self.preferences.set_game_tier.call(AppId(app_id), tier)
    }

    pub(crate) fn appears_online(&self, online: bool) -> Result<(), PreferencesError> {
        self.preferences.set_appear_online.call(online)
    }

    pub(crate) fn farms_only_priority(&self, only: bool) -> Result<(), PreferencesError> {
        self.preferences.set_only_priority.call(only)
    }

    /// Quits steamcards and starts it again.
    pub(crate) fn comes_back(self) -> Self {
        let preferences = component(&self.config);
        Self {
            preferences,
            ..self
        }
    }

    /// The disk fills up: nothing more can be written to the config file.
    pub(crate) fn runs_out_of_disk(&self) {
        let _ = fs::remove_file(&self.config);
        fs::create_dir_all(&self.config).unwrap();
    }
}

fn component(config: &std::path::Path) -> PreferencesComponent {
    PreferencesComponent::new(Arc::new(ConfigFile::open(config.to_path_buf()).unwrap()))
}
