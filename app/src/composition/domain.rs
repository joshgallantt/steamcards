use account_di::AccountComponent;
use card_di::CardComponent;
use farming_di::FarmingComponent;
use game_di::GameComponent;
use market_di::MarketComponent;
use preferences_di::PreferencesComponent;

use super::DataAssembler;

/// Phase two: each component's use cases, over phase one's stores.
pub(crate) struct DomainAssembler {
    pub account: AccountComponent,
    pub game: GameComponent,
    pub preferences: PreferencesComponent,
    pub farming: FarmingComponent,
    pub market: MarketComponent,
}

impl DomainAssembler {
    pub(crate) fn new(data: &DataAssembler) -> Self {
        let account = AccountComponent::new(data.steam.clone());
        let game = GameComponent::new(data.steam.clone());
        // Only the farmer looks at cards: it takes their use cases here.
        let card = CardComponent::new(data.steam.clone());
        let preferences = PreferencesComponent::new(data.config.clone());
        let farming = FarmingComponent::new(
            data.steam.clone(),
            data.awake.clone(),
            game.read.clone(),
            card.look_at.clone(),
            card.look_at_foils.clone(),
            card.describe.clone(),
            preferences.get.clone(),
        );
        let market =
            MarketComponent::new(data.steam.clone(), data.config.clone(), data.prices.clone());
        Self {
            account,
            game,
            preferences,
            farming,
            market,
        }
    }
}
