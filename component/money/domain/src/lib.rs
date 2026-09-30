//! Money: amounts as Steam counts them, in hundredths of a currency's unit,
//! with the currency Steam names by its `ECurrency` id; and as Steam writes
//! them. Prices, the wallet and the screens all count in it, so it's a
//! component of its own, as the rules of exact amounts are: amounts in two
//! currencies are never added or converted.
//!
//! Every currency's format is Valve's own, from `g_rgCurrencyData` in
//! steamcommunity.com's global.js, and an amount is written as that page's
//! `v_currencyformat` writes it (research: market-and-session.md, §1.2).

mod model;

pub use model::{Currency, Money};
