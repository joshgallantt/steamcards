use std::time::Duration;

use terminal_ui::{
    App,
    viewmodel::{Account, Farming, Games, Library, Login, Market, Onboarding},
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
            Account::new(
                d.account.get_account.clone(),
                d.account.check_sign_in.clone(),
                d.account.sign_out.clone(),
            ),
            Login::new(d.account.sign_in.clone()),
            Farming::new(
                d.farming.farm_cards.clone(),
                d.session.end_session.clone(),
                d.account.get_account.clone(),
                d.preferences.get_preferences.clone(),
                d.preferences.set_game_tier.clone(),
            ),
            Games::new(
                d.preferences.get_preferences.clone(),
                d.preferences.set_game_tier.clone(),
                d.preferences.set_only_priority.clone(),
                d.preferences.set_appear_online.clone(),
            ),
            Library::new(d.game.read_library.clone()),
            Onboarding::new(d.account.get_account.clone()),
            Market::new(
                d.price.get_prices.clone(),
                d.price.set_games_to_price.clone(),
                d.price.keep_prices_up_to_date.clone(),
                d.price.refresh_prices.clone(),
                d.price.get_wallet.clone(),
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
