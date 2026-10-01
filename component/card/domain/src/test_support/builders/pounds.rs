use account::Wallet;
use money::Currency;

/// A wallet in pounds, with Valve's fees.
pub fn pounds() -> Wallet {
    Wallet::new(Currency::GBP)
}
