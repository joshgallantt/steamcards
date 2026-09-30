use account_di::AccountComponent;
use farming_di::FarmingComponent;
use library_di::LibraryComponent;
use preferences_di::PreferencesComponent;

use super::DataAssembler;

/// Phase two: each component's use cases, over phase one's stores.
pub(crate) struct DomainAssembler {
    pub account: AccountComponent,
    pub library: LibraryComponent,
    pub preferences: PreferencesComponent,
    pub farming: FarmingComponent,
}

impl DomainAssembler {
    pub(crate) fn new(data: &DataAssembler) -> Self {
        let account = AccountComponent::new(data.steam.clone());
        let library = LibraryComponent::new(data.steam.clone());
        let preferences = PreferencesComponent::new(data.config.clone());
        let farming = FarmingComponent::new(
            data.steam.clone(),
            data.awake.clone(),
            library.read.clone(),
            library.look_at.clone(),
            library.describe.clone(),
            preferences.get.clone(),
        );
        Self {
            account,
            library,
            preferences,
            farming,
        }
    }
}
