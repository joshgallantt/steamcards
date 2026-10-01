use std::sync::Arc;

use crate::sign_in::SignInUpdate;

use account::{LoginChallenge, SignInUseCase};
use tokio::sync::mpsc;

/// Signing in with the Steam app, in the background: QR codes to show while
/// it waits, then how it went.
pub struct SignInViewModel {
    sign_in: Arc<dyn SignInUseCase>,
    /// Stops the sign-in itself on cancel.
    task: Option<tokio::task::AbortHandle>,
    updates: Option<mpsc::UnboundedReceiver<SignInUpdate>>,
}

impl SignInViewModel {
    pub fn new(sign_in: Arc<dyn SignInUseCase>) -> Self {
        Self {
            sign_in,
            task: None,
            updates: None,
        }
    }

    /// Starts signing in; progress arrives through `try_recv`.
    pub fn start(&mut self) {
        self.cancel();
        let (tx, rx) = mpsc::unbounded_channel::<SignInUpdate>();
        self.updates = Some(rx);
        let (challenge_tx, mut challenge_rx) = mpsc::unbounded_channel::<LoginChallenge>();

        // Passes on each QR code as it comes; ends with the sign-in.
        let codes = tx.clone();
        tokio::spawn(async move {
            while let Some(c) = challenge_rx.recv().await {
                let _ = codes.send(SignInUpdate {
                    challenge: Some(c),
                    done: false,
                    err: None,
                });
            }
        });

        let handle = self.sign_in.call(challenge_tx);
        self.task = Some(handle.abort_handle());
        tokio::spawn(async move {
            let err = match handle.await {
                Ok(Ok(())) => None,
                Ok(Err(e)) => Some(e.to_string()),
                Err(e) if e.is_cancelled() => return,
                Err(e) => Some(format!("the sign-in stopped: {e}")),
            };
            let _ = tx.send(SignInUpdate {
                challenge: None,
                done: true,
                err,
            });
        });
    }

    pub fn try_recv(&mut self) -> Option<SignInUpdate> {
        self.updates.as_mut()?.try_recv().ok()
    }

    pub fn cancel(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
        self.updates = None;
    }
}
