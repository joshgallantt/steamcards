//! Presentation without a screen: farms, and prints every event as a line of
//! plain text, for logs and servers, in the dashboard's words.

#![expect(
    clippy::print_stdout,
    reason = "printing to stdout is this presentation's whole job"
)]

use std::{sync::Arc, time::Duration};

use account::GetAccountUseCase;
use farming::{FarmCardsUseCase, FarmingStatus, FarmingUpdate};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use update::{KeepUpToDateUseCase, UpdateEvent};

/// Farms until `duration` passes (never, if `None`) or ctrl-c, keeping
/// steamcards up to date meanwhile. Signing in takes the Steam app and a
/// screen for its QR code, so it's done in the TUI first.
pub async fn run(
    account: Arc<dyn GetAccountUseCase>,
    farm: Arc<dyn FarmCardsUseCase>,
    keep_up_to_date: Arc<dyn KeepUpToDateUseCase>,
    duration: Option<Duration>,
) {
    let Some(signed_in) = account.call() else {
        println!(
            "Not signed in to Steam. Run steamcards without --headless once, and sign in with \
             the Steam app."
        );
        return;
    };
    if signed_in.expired {
        println!("Steam no longer takes the saved sign-in: run steamcards to sign in again.");
        return;
    }
    let token = CancellationToken::new();
    if let Some(d) = duration {
        let t = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(d).await;
            t.cancel();
        });
    }
    {
        let t = token.clone();
        tokio::spawn(async move {
            let _ = tokio::signal::ctrl_c().await;
            t.cancel();
        });
    }

    let (found_tx, mut found) = mpsc::channel::<UpdateEvent>(8);
    drop(keep_up_to_date.call(token.clone(), found_tx));
    let announcer = tokio::spawn(async move {
        while let Some(f) = found.recv().await {
            let ts = chrono::Local::now().format("%H:%M:%S");
            println!("{ts} {}", farming_words::update(&f));
        }
    });

    let (tx, mut rx) = mpsc::channel::<FarmingUpdate>(256);
    let printer = tokio::spawn(async move {
        let mut last = None;
        while let Some(update) = rx.recv().await {
            print_update(&update, &mut last);
        }
    });
    let _ = farm.call(token.clone(), tx).await;
    // Farming can stop by itself, as when another session takes over: the
    // updates stop with it.
    token.cancel();
    let _ = printer.await;
    let _ = announcer.await;
}

/// What a status line says, to print it only when that changes.
type Said = (String, Vec<String>, String);

fn print_update(update: &FarmingUpdate, last: &mut Option<Said>) {
    let ts = chrono::Local::now().format("%H:%M:%S");
    match update {
        FarmingUpdate::Status(s) => {
            let said = said(s);
            if last.as_ref() != Some(&said) {
                let (status, playing, note) = &said;
                let playing = if playing.is_empty() {
                    String::new()
                } else {
                    format!(" playing={}", playing.join(", "))
                };
                let note = if note.is_empty() {
                    String::new()
                } else {
                    format!(" ({note})")
                };
                println!("{ts} STATUS {status}{playing}{note}");
                *last = Some(said);
            }
        }
        FarmingUpdate::Event(e) => println!("{ts} {}", farming_words::event(e)),
    }
}

fn said(s: &FarmingStatus) -> Said {
    let playing = s
        .playing
        .iter()
        .map(|&id| {
            s.library
                .game(id)
                .map_or_else(|| format!("app {id}"), |g| g.name.clone())
        })
        .collect();
    let note = farming_words::note(s, chrono::Utc::now()).unwrap_or_default();
    (farming_words::status(s.status).to_owned(), playing, note)
}
