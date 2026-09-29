use account::{Account as SignedIn, GetAccount, RefreshAccount, UnlinkAccount, UnlinkError};

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
