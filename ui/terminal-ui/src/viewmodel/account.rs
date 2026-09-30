use std::sync::Arc;

use account::{
    Account as SignedIn, CheckSignInUseCase, GetAccountUseCase, SignOutError, SignOutUseCase,
};

/// The one Steam account: who's signed in, and signing out.
pub struct Account {
    get_account: Arc<dyn GetAccountUseCase>,
    check_sign_in: Arc<dyn CheckSignInUseCase>,
    sign_out: Arc<dyn SignOutUseCase>,
}

impl Account {
    /// Also starts a background check of the saved sign-in.
    pub fn new(
        get_account: Arc<dyn GetAccountUseCase>,
        check_sign_in: Arc<dyn CheckSignInUseCase>,
        sign_out: Arc<dyn SignOutUseCase>,
    ) -> Self {
        check_sign_in.call();
        Self {
            get_account,
            check_sign_in,
            sign_out,
        }
    }

    /// Who's signed in, if anyone.
    pub fn get(&self) -> Option<SignedIn> {
        self.get_account.call()
    }

    pub fn is_signed_in(&self) -> bool {
        self.get().is_some()
    }

    /// Re-checks the saved sign-in in the background (e.g. after signing in).
    pub fn refresh(&self) {
        self.check_sign_in.call();
    }

    /// Signs out: the saved sign-in is forgotten.
    pub fn sign_out(&self) -> Result<(), SignOutError> {
        self.sign_out.call()
    }
}

#[cfg(test)]
mod tests {
    use account::test_support::{SpyCheckSignInUseCase, SpySignOutUseCase, StubGetAccountUseCase};

    use super::*;

    fn account(check: Arc<SpyCheckSignInUseCase>, sign_out: Arc<SpySignOutUseCase>) -> Account {
        Account::new(Arc::new(StubGetAccountUseCase::new(None)), check, sign_out)
    }

    #[test]
    fn the_saved_sign_in_is_checked_from_the_start() {
        let check = Arc::new(SpyCheckSignInUseCase::default());
        account(check.clone(), Arc::default());
        assert_eq!(check.checks(), 1);
    }

    #[test]
    fn signing_out_is_passed_on_to_the_account() {
        let sign_out = Arc::new(SpySignOutUseCase::default());
        account(Arc::default(), sign_out.clone())
            .sign_out()
            .unwrap();
        assert_eq!(sign_out.sign_outs(), 1);
    }
}
