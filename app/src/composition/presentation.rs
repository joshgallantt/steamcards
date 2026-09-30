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
                d.account.get.clone(),
                d.account.refresh.clone(),
                d.account.unlink.clone(),
            ),
            Login::new(d.account.link.clone()),
            Farming::new(
                d.farming.farm.clone(),
                d.farming.end_session.clone(),
                d.account.get.clone(),
                d.preferences.get.clone(),
                d.preferences.set_game_tier.clone(),
            ),
            Games::new(
                d.preferences.get.clone(),
                d.preferences.set_game_tier.clone(),
                d.preferences.set_only_priority.clone(),
                d.preferences.set_appear_online.clone(),
            ),
            Library::new(d.game.read.clone()),
            Onboarding::new(d.account.get.clone()),
            Market::new(
                d.price.prices.clone(),
                d.price.want.clone(),
                d.price.watch.clone(),
                d.price.refresh.clone(),
                d.price.wallet.clone(),
            ),
        )
    }

    pub(crate) async fn run_headless(&self, duration: Option<Duration>) {
        let d = &self.domain;
        headless::run(d.account.get.clone(), d.farming.farm.clone(), duration).await;
    }
}
