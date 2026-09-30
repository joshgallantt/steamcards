use tokio::{sync::mpsc, task::JoinHandle};

use crate::{LoginChallenge, SignInError, SignInUseCase};

/// Refuses every sign-in, so nothing reaches Steam.
pub struct StubSignInUseCase;

impl SignInUseCase for StubSignInUseCase {
    fn call(
        &self,
        _: mpsc::UnboundedSender<LoginChallenge>,
    ) -> JoinHandle<Result<(), SignInError>> {
        tokio::spawn(async { Err(SignInError::Refused("not in a test".into())) })
    }
}
