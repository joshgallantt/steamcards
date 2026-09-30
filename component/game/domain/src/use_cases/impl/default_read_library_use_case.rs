use std::sync::Arc;

use tokio::task::JoinHandle;

use crate::{GameError, GameRepository, ReadLibraryUseCase, SteamLibrary};

pub struct DefaultReadLibraryUseCase {
    repo: Arc<dyn GameRepository>,
}

impl DefaultReadLibraryUseCase {
    pub fn new(repo: Arc<dyn GameRepository>) -> Self {
        Self { repo }
    }
}

impl ReadLibraryUseCase for DefaultReadLibraryUseCase {
    fn call(&self) -> JoinHandle<Result<SteamLibrary, GameError>> {
        let repo = Arc::clone(&self.repo);
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
    }
}
