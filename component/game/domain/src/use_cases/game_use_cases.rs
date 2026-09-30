//! What can be asked of the games. Each use case is a value: its type says
//! what it takes and gives, and a constructor builds the real one over the
//! repository.

use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::{GameError, GameRepository, SteamLibrary};

/// Reads the library in the background: games with drops left first, most
/// played first, then finished ones, by name.
pub type ReadLibrary = Arc<dyn Fn() -> JoinHandle<Result<SteamLibrary, GameError>> + Send + Sync>;

pub fn read_library(repo: Arc<dyn GameRepository>) -> ReadLibrary {
    Arc::new(move || {
        let repo = Arc::clone(&repo);
        tokio::spawn(async move {
            let library = repo
                .library()
                .await
                .map_err(|e| GameError::Unavailable(e.to_string()))?;
            let mut games = library.games().to_vec();
            games.sort_by(|a, b| {
                b.has_drops_left()
                    .cmp(&a.has_drops_left())
                    .then(b.hours.total_cmp(&a.hours))
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });
            Ok(SteamLibrary::new(games))
        })
    })
}
