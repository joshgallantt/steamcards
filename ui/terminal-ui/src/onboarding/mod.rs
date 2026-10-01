//! Getting set up on the first run: a welcome, signing in, picking games,
//! starting to farm. Its view model, and the screens of each step.

mod needs_account;
pub(crate) mod onboarding_view;
mod onboarding_view_model;
mod step;

pub use needs_account::NeedsAccount;
pub use onboarding_view_model::OnboardingViewModel;
pub use step::Step;
