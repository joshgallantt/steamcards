use account_di::AccountComponent;
use card_di::CardComponent;
use farming_di::FarmingComponent;
use game_di::GameComponent;
use preferences_di::PreferencesComponent;
use price_di::PriceComponent;
use session_di::SessionComponent;

use super::DataAssembler;

/// Phase two: each component's use cases, over phase one's stores.
pub(crate) struct DomainAssembler {
    pub account: AccountComponent,
    pub game: GameComponent,
    pub preferences: PreferencesComponent,
    pub session: SessionComponent,
    pub farming: FarmingComponent,
    pub price: PriceComponent,
}

impl DomainAssembler {
    pub(crate) fn new(data: &DataAssembler) -> Self {
        let account = AccountComponent::new(data.steam.clone());
        let game = GameComponent::new(data.steam.clone());
        // Only the farmer looks at cards: it takes their use cases here.
        let card = CardComponent::new(data.steam.clone());
        let preferences = PreferencesComponent::new(data.config.clone());
        let session = SessionComponent::new(data.sessions.clone());
        let farming = FarmingComponent::new(
            data.steam.clone(),
            game.get_library.clone(),
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
            game,
            preferences,
            session,
            farming,
            price,
        }
    }
}
