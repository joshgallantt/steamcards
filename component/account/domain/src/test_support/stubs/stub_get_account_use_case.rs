use std::sync::Mutex;

use crate::{Account, GetAccountUseCase};

/// Answers with whoever was last set: someone signed in, or nobody.
pub struct StubGetAccountUseCase {
    account: Mutex<Option<Account>>,
}

impl StubGetAccountUseCase {
    pub fn new(account: Option<Account>) -> Self {
        Self {
            account: Mutex::new(account),
        }
    }

    pub fn set(&self, account: Option<Account>) {
        *self.account.lock().unwrap() = account;
    }
}

impl GetAccountUseCase for StubGetAccountUseCase {
    fn call(&self) -> Option<Account> {
        self.account.lock().unwrap().clone()
    }
}
