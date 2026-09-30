//! Everything asked of the session, a trait each. Each is done over the
//! session's keeper by a `Default…UseCase` in `impl/`.

/// Ends the farming session: the farmer's next run starts a new one, with no
/// drops, no hours counted and nothing set aside. Signing out ends it, and
/// so does signing in as another account.
pub trait EndSessionUseCase: Send + Sync {
    fn call(&self);
}
