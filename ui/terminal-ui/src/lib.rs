//! Presentation: the full-screen terminal dashboard, a feature to a module.
//! Each holds its view models, which turn use cases into screen state and
//! keys into use case calls, and beside them the views that draw that state.
//! The app holds them all, draws what's on screen and forwards keys. Nothing
//! here knows a repository exists: this crate depends on domain crates only.

pub mod account;
mod app;
pub mod dashboard;
pub mod games;
mod help;
pub mod onboarding;
mod popup;
pub mod settings;
pub mod sign_in;
mod theme;
mod widgets;

pub use app::App;
