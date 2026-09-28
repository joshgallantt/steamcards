use crate::Preferences;

/// Where preferences are kept. Declared here, beside the use cases that need
/// it; the data layer is written to fit.
pub trait PreferencesRepository: Send + Sync {
    fn preferences(&self) -> Preferences;

    /// Errs when the preferences could not be kept, so a caller cannot report
    /// a change that did not happen. What `preferences` returns afterwards is
    /// what was kept.
    fn save(&self, preferences: Preferences) -> anyhow::Result<()>;
}
