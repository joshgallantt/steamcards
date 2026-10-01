use std::time::Duration;

use terminal_ui::{
    App,
    account::AccountViewModel,
    dashboard::{FarmingViewModel, LibraryViewModel, MarketViewModel},
    games::GamesViewModel,
    onboarding::OnboardingViewModel,
    sign_in::SignInViewModel,
};

use super::DomainAssembler;

/// Phase three: screens, from use cases only.
pub(crate) struct PresentationAssembler {
    domain: DomainAssembler,
}

impl PresentationAssembler {
    pub(crate) fn new(domain: DomainAssembler) -> Self {
        Self { domain }
    }

    pub(crate) fn make_terminal_app(&self) -> App {
        let d = &self.domain;
        App::new(
            AccountViewModel::new(
                d.account.get_account.clone(),
                d.account.check_sign_in.clone(),
                d.account.sign_out.clone(),
            ),
            SignInViewModel::new(d.account.sign_in.clone()),
            FarmingViewModel::new(
                d.farming.farm_cards.clone(),
                d.session.end_session.clone(),
                d.account.get_account.clone(),
                d.preferences.get_preferences.clone(),
                d.preferences.set_game_tier.clone(),
            ),
            GamesViewModel::new(
                d.preferences.get_preferences.clone(),
                d.preferences.set_game_tier.clone(),
                d.preferences.set_only_priority.clone(),
                d.preferences.set_appear_online.clone(),
            ),
            LibraryViewModel::new(d.game.get_library.clone()),
            OnboardingViewModel::new(d.account.get_account.clone()),
            MarketViewModel::new(
                d.card.get_card_prices.clone(),
                d.card.set_cards_to_price.clone(),
                d.card.keep_card_prices_up_to_date.clone(),
                d.card.refresh_card_prices.clone(),
                d.account.get_wallet.clone(),
            ),
        )
    }

    pub(crate) async fn run_headless(&self, duration: Option<Duration>) {
        let d = &self.domain;
        headless::run(
            d.account.get_account.clone(),
            d.farming.farm_cards.clone(),
            duration,
        )
        .await;
    }
}
