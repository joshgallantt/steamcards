use account_di::AccountComponent;
use card_di::CardComponent;
use farming::FarmingDependencies;
use farming_di::FarmingComponent;
use game_di::GameComponent;
use preferences_di::PreferencesComponent;
use session_di::SessionComponent;

use super::DataAssembler;

/// Phase two: each component's use cases, over phase one's stores.
pub(crate) struct DomainAssembler {
    pub account: AccountComponent,
    pub game: GameComponent,
    pub preferences: PreferencesComponent,
    pub session: SessionComponent,
    pub card: CardComponent,
    pub farming: FarmingComponent,
}

impl DomainAssembler {
    pub(crate) fn new(data: &DataAssembler) -> Self {
        let account = AccountComponent::new(data.steam.clone());
        let game = GameComponent::new(data.steam.clone());
        let card = CardComponent::new(data.steam.clone(), data.config.clone(), data.prices.clone());
        let preferences = PreferencesComponent::new(data.config.clone());
        let session = SessionComponent::new(data.sessions.clone());
        // The farmer plays and looks through the game and card components'
        // use cases, and keeps nothing of its own.
        let farming = FarmingComponent::new(FarmingDependencies {
            get_library: game.get_library.clone(),
            play_games: game.play_games.clone(),
            stand_by: game.stand_by.clone(),
            stop_playing: game.stop_playing.clone(),
            observe_playing: game.observe_playing.clone(),
            look_at_cards: card.look_at_cards.clone(),
            look_at_foils: card.look_at_foils.clone(),
            identify_cards: card.identify_cards.clone(),
            observe_new_items: card.observe_new_items.clone(),
            get_preferences: preferences.get_preferences.clone(),
            sessions: data.sessions.clone(),
        });
        Self {
            account,
            game,
            preferences,
            session,
            card,
            farming,
        }
    }
}
