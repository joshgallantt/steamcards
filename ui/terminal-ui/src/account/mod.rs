//! The account: who's signed in, and signing out, and keeping steamcards
//! up to date. Their view models, and the pop-up that shows who's signed
//! in.

pub(crate) mod account_view;
mod account_view_model;
mod update_view_model;

pub use account_view_model::AccountViewModel;
pub use update_view_model::UpdateViewModel;
