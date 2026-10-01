use money::Currency;

use crate::Wallet;

/// A wallet in pounds, with Valve's fees.
pub fn pounds() -> Wallet {
    Wallet::new(Currency::GBP)
}
