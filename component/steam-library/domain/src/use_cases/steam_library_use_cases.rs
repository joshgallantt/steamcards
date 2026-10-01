//! Everything asked of the library, a trait each. Each is done over the
//! repository by a `Default…UseCase` in `impl/`.

use tokio::task::JoinHandle;

use crate::{SteamLibrary, SteamLibraryError};

/// Reads the library in the background: games with drops left first, most
/// played first, then finished ones, by name.
pub trait ReadLibraryUseCase: Send + Sync {
    fn call(&self) -> JoinHandle<Result<SteamLibrary, SteamLibraryError>>;
}
