//! Every game with cards, for browsing: the onboarding's games step and the
//! games pop-up, whether farming has started or not.

use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::{CardsRepository, Library};

/// Lists every game with cards in the background; the handle resolves with
/// them, games with cards left first (most hours first), then finished ones,
/// by name.
pub type ListGames = Arc<dyn Fn() -> JoinHandle<Library> + Send + Sync>;

pub fn list_games(repo: Arc<dyn CardsRepository>) -> ListGames {
    Arc::new(move || {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            match repo.games().await {
                Ok(mut games) => {
                    games.sort_by(|a, b| {
                        a.is_done()
                            .cmp(&b.is_done())
                            .then(b.hours.total_cmp(&a.hours))
                            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
                    });
                    Library {
                        games,
                        failed: None,
                    }
                }
                Err(e) => Library {
                    games: Vec::new(),
                    failed: Some(e.to_string()),
                },
            }
        })
    })
}
