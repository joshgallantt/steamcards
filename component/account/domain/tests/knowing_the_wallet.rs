//! Unit tier: the account's wallet, its use case over a fake repository.
//! The acceptance tier, through the account component and a stand-in for
//! Steam, is in account-di.

use std::sync::Arc;

use account::{
    DefaultGetWalletUseCase, GetWalletUseCase, Wallet, test_support::FakeAccountRepository,
};
use money::Currency;

#[test]
fn the_wallet_is_whatever_steam_last_said() {
    let steam = Arc::new(FakeAccountRepository::signed_in("Ellie"));
    let wallet = DefaultGetWalletUseCase::new(steam.clone());
    assert_eq!(wallet.call(), None, "not signed on yet");

    *steam.wallet.lock().unwrap() = Some(Wallet::new(Currency::GBP));

    let pounds = wallet.call().unwrap();
    assert_eq!(pounds.currency, Currency::GBP);
    assert_eq!(pounds.seller_gets(62), 55, "Valve's fees");
}
