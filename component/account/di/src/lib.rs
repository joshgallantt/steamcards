//! Where the account domain meets its data layer: one repository over the
//! Steam session, handed to every use case. The composition root names the
//! session.

use std::sync::Arc;

use account::{
    AccountRepository, GetAccount, LinkAccount, RefreshAccount, UnlinkAccount, get_account,
    link_account, refresh_account, unlink_account,
};
use account_data::SteamAccountRepository;
use steam_api::Session;

pub struct AccountComponent {
    pub get: GetAccount,
    pub refresh: RefreshAccount,
    pub link: LinkAccount,
    pub unlink: UnlinkAccount,
}

impl AccountComponent {
    pub fn new(session: Arc<Session>) -> Self {
        Self::over(Arc::new(SteamAccountRepository::new(session)))
    }

    pub fn over(repo: Arc<dyn AccountRepository>) -> Self {
        Self {
            get: get_account(repo.clone()),
            refresh: refresh_account(repo.clone()),
            link: link_account(repo.clone()),
            unlink: unlink_account(repo),
        }
    }
}
