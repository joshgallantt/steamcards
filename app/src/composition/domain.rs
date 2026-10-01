use account_di::AccountComponent;
use card_di::CardComponent;
use farming_di::FarmingComponent;
use preferences_di::PreferencesComponent;
use price_di::PriceComponent;
use session_di::SessionComponent;
use steam_library_di::SteamLibraryComponent;

use super::DataAssembler;

/// Phase two: each component's use cases, over phase one's stores.
pub(crate) struct DomainAssembler {
    pub account: AccountComponent,
    pub steam_library: SteamLibraryComponent,
    pub preferences: PreferencesComponent,
    pub session: SessionComponent,
    pub farming: FarmingComponent,
    pub price: PriceComponent,
}

impl DomainAssembler {
    pub(crate) fn new(data: &DataAssembler) -> Self {
        let account = AccountComponent::new(data.steam.clone());
        let steam_library = SteamLibraryComponent::new(data.steam.clone());
        // Only the farmer looks at cards: it takes their use cases here.
        let card = CardComponent::new(data.steam.clone());
        let preferences = PreferencesComponent::new(data.config.clone());
        let session = SessionComponent::new(data.sessions.clone());
        let farming = FarmingComponent::new(
            data.steam.clone(),
            steam_library.read_library.clone(),
            card.look_at_cards.clone(),
            card.look_at_foils.clone(),
            card.identify_cards.clone(),
            preferences.get_preferences.clone(),
            data.sessions.clone(),
        );
        let price =
            PriceComponent::new(data.steam.clone(), data.config.clone(), data.prices.clone());
        Self {
            account,
            steam_library,
            preferences,
            session,
            farming,
            price,
        }
    }
}
