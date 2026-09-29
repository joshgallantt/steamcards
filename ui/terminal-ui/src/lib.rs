//! Presentation: the full-screen terminal dashboard.
//!
//! `viewmodel` turns use cases into screen state and screen actions into use
//! case calls; `tui` draws that state and forwards keys. Neither knows a
//! repository exists — this crate depends on domain crates only.

mod tui;
pub mod viewmodel;

pub use tui::App;
