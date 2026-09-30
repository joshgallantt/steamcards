//! Everything the user asks of their account, a trait each. Each is done
//! over the repository by a `Default…UseCase` in `impl/`. A view model holds
//! only the ones it calls, and a test double is a type of its own in
//! `test_support`.

use tokio::{sync::mpsc, task::JoinHandle};

use crate::{Account, LoginChallenge, SignInError, SignOutError};

/// The signed-in account, or `None` when nobody is signed in.
pub trait GetAccountUseCase: Send + Sync {
    fn call(&self) -> Option<Account>;
}

/// Checks the saved sign-in again in the background; `GetAccountUseCase`
/// answers with the result once it lands.
pub trait CheckSignInUseCase: Send + Sync {
    fn call(&self);
}

/// Starts signing in. Challenges arrive on the channel; the handle resolves
/// when the sign-in ends, and aborting it stops the sign-in.
pub trait SignInUseCase: Send + Sync {
    fn call(
        &self,
        challenges: mpsc::UnboundedSender<LoginChallenge>,
    ) -> JoinHandle<Result<(), SignInError>>;
}

/// Signs out: the saved sign-in is forgotten, so nothing farms until the
/// account signs in again.
pub trait SignOutUseCase: Send + Sync {
    fn call(&self) -> Result<(), SignOutError>;
}
