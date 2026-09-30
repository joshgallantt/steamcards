use account::{Account as SignedIn, GetAccount, RefreshAccount, UnlinkAccount, UnlinkError};
use market::Currency;

use super::screen::{Run, Snapshot};

/// The one Steam account: who's signed in, and signing out.
pub struct Account {
    get: GetAccount,
    refresh: RefreshAccount,
    unlink: UnlinkAccount,
}

impl Account {
    /// Also starts a background check of the saved sign-in.
    pub fn new(get: GetAccount, refresh: RefreshAccount, unlink: UnlinkAccount) -> Self {
        refresh();
        Self {
            get,
            refresh,
            unlink,
        }
    }

    /// Who's signed in, if anyone.
    pub fn get(&self) -> Option<SignedIn> {
        (self.get)()
    }

    pub fn is_signed_in(&self) -> bool {
        self.get().is_some()
    }

    /// Re-checks the saved sign-in in the background (e.g. after signing in).
    pub fn refresh(&self) {
        (self.refresh)();
    }

    /// Signs out: the saved sign-in is forgotten.
    pub fn sign_out(&self) -> Result<(), UnlinkError> {
        (self.unlink)()
    }
}

/// The account pop-up: who's signed in and whether Steam still takes the
/// sign-in, how friends see the games, what farming is doing, the wallet's
/// currency every price is shown in, and that the computer is kept awake
/// while games play.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountView {
    /// The account's name; `None` when nobody is signed in, and empty when
    /// Steam didn't say it.
    pub name: Option<String>,
    pub expired: bool,
    pub appear_online: bool,
    pub run: Run,
    /// The wallet's currency, once Steam has said.
    pub currency: Option<Currency>,
    pub keeping_awake: bool,
}

impl AccountView {
    pub fn build(s: &Snapshot<'_>) -> Self {
        Self {
            name: s.account.map(|a| a.name.clone()),
            expired: s.account.is_some_and(|a| a.expired),
            appear_online: s.prefs.appear_online,
            run: s.run,
            currency: s.wallet.map(|w| w.currency),
            keeping_awake: !s.playing().is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewmodel::fixtures;

    #[test]
    fn the_account_and_its_wallet() {
        let data = fixtures::farming_alone();
        let view = AccountView::build(&data.snapshot());
        assert_eq!(view.name.as_deref(), Some("alice"));
        assert!(!view.expired && !view.appear_online && view.keeping_awake);
        assert_eq!(view.run, Run::Running);
        let currency = view.currency.unwrap();
        assert_eq!(
            (currency.symbol(), currency.code()),
            (Some("£"), Some("GBP")),
            "Wallet £ GBP"
        );

        let expired = fixtures::sign_in_expired();
        let view = AccountView::build(&expired.snapshot());
        assert!(view.expired && !view.keeping_awake);
    }
}
