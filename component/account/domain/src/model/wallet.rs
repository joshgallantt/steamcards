use money::Currency;

/// The account's Steam wallet, as far as the market goes: its currency, and
/// the fees Steam takes from a sale. Amounts are in hundredths of its
/// currency, fees in basis points (500 is 5%).
///
/// The fee rules are Valve's own, from economy_common.js (research §1.4), in
/// whole numbers: they give what Valve's floating-point ones give at every
/// price up to 200,000 hundredths, with the defaults and with other
/// minimums, steps and fees.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wallet {
    pub currency: Currency,
    /// The least a seller may get, and the least any fee is:
    /// `wallet_market_minimum`.
    pub market_minimum: i64,
    /// Prices go up in steps of this: `wallet_currency_increment`.
    pub increment: i64,
    /// Steam's fee, in basis points: `wallet_fee_percent`.
    pub steam_fee: u32,
    /// The game's publisher's fee, in basis points:
    /// `wallet_publisher_fee_percent_default`.
    pub publisher_fee: u32,
    /// The most a buyer may pay: `wallet_trade_max_balance`. `None` while it
    /// isn't known.
    pub trade_max: Option<i64>,
}

impl Wallet {
    /// A wallet in `currency`, with Valve's defaults: a minimum of 1, steps
    /// of 1, 5% for Steam and 10% for the publisher. They gave the right
    /// answer in every example the research checked.
    pub fn new(currency: Currency) -> Self {
        Self {
            currency,
            market_minimum: 1,
            increment: 1,
            steam_fee: 500,
            publisher_fee: 1_000,
            trade_max: None,
        }
    }

    /// What a buyer pays for a card listed to pay its seller `seller_gets`:
    /// Valve's `GetTotalWithFees`.
    pub fn buyer_pays(&self, seller_gets: i64) -> i64 {
        self.valid(seller_gets)
            + self.fee(seller_gets, self.publisher_fee)
            + self.fee(seller_gets, self.steam_fee)
    }

    /// What the seller gets when a buyer pays `buyer_pays`: Valve's
    /// `GetItemPriceFromTotal`. Some buyer prices can't occur: 22p pays 19p,
    /// and a card listed to pay 19p costs a buyer 21p. So a price to list
    /// at is `buyer_pays` of what the seller is to get.
    pub fn seller_gets(&self, buyer_pays: i64) -> i64 {
        let with_fees = 10_000 + i64::from(self.publisher_fee) + i64::from(self.steam_fee);
        let guess = buyer_pays.saturating_mul(10_000).div_euclid(with_fees);
        let mut base = self.valid(guess.min(buyer_pays - 2 * self.market_minimum));
        // At or under the answer now; it's at most a few steps up.
        for _ in 0..3 {
            let total = self.buyer_pays(base);
            if total == buyer_pays {
                return base;
            }
            if total < buyer_pays {
                base += self.increment;
            } else {
                base -= self.increment;
                break;
            }
        }
        base.max(self.market_minimum)
    }

    /// Valve's `ToValidMarketPrice`: at least the minimum, and a whole
    /// number of steps, rounded to the nearest.
    fn valid(&self, price: i64) -> i64 {
        if price <= self.market_minimum {
            return self.market_minimum;
        }
        if price <= self.increment {
            return self.increment;
        }
        if self.increment > 1 {
            let steps = (2 * price.abs() + self.increment) / (2 * self.increment);
            return price.signum() * steps * self.increment;
        }
        price
    }

    /// Valve's `CalculateFee`: a share of `base`, rounded down, then made a
    /// valid price. A share of nothing is no fee at all.
    fn fee(&self, base: i64, basis_points: u32) -> i64 {
        if basis_points == 0 {
            return 0;
        }
        self.valid(
            base.saturating_mul(i64::from(basis_points))
                .div_euclid(10_000),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pounds() -> Wallet {
        Wallet::new(Currency::GBP)
    }

    /// What undercutting lists at, and pays: a step under the lowest
    /// listing, never under the best offer (ui.md §7).
    fn undercut(wallet: &Wallet, ask: i64, bid: i64) -> (i64, i64) {
        let gets = (wallet.seller_gets(ask) - wallet.increment).max(wallet.seller_gets(bid));
        (wallet.buyer_pays(gets), gets)
    }

    #[test]
    fn a_seller_gets_what_a_buyer_pays_less_steams_fees() {
        // research §1.4, and ui.md §7's 62p.
        let table = [
            (3, 1),
            (4, 2),
            (5, 3),
            (6, 4),
            (8, 6),
            (11, 9),
            (13, 11),
            (23, 20),
            (29, 26),
            (62, 55),
            (63, 56),
            (100, 88),
        ];
        let wallet = pounds();
        for (buyer, seller) in table {
            assert_eq!(wallet.seller_gets(buyer), seller, "a buyer paying {buyer}p");
            assert_eq!(
                wallet.buyer_pays(seller),
                buyer,
                "a seller getting {seller}p"
            );
        }
    }

    #[test]
    fn some_buyer_prices_cant_occur() {
        let wallet = pounds();
        assert_eq!(wallet.seller_gets(22), 19);
        assert_eq!(wallet.buyer_pays(19), 21, "19p lists at 21p, not 22p");
        let never: Vec<i64> = (3..200)
            .filter(|&t| wallet.buyer_pays(wallet.seller_gets(t)) != t)
            .collect();
        assert_eq!(never.len(), 23, "{never:?}");
        assert!(
            (1..5_000).all(|b| wallet.seller_gets(wallet.buyer_pays(b)) == b),
            "every seller amount lists at a price that pays it"
        );
    }

    #[test]
    fn undercutting_lists_a_step_under_the_lowest_listing_never_under_the_best_offer() {
        // ui.md §7: Thanatos, Nyx, Zagreus, Madison and The Boy.
        let wallet = pounds();
        assert_eq!(undercut(&wallet, 62, 41), (61, 54));
        assert_eq!(undercut(&wallet, 9, 7), (8, 6));
        assert_eq!(undercut(&wallet, 8, 6), (7, 5));
        assert_eq!(undercut(&wallet, 5, 4), (4, 2));
        assert_eq!(undercut(&wallet, 4, 3).0, 3, "The Boy lists at 3p");
    }

    #[test]
    fn prices_keep_to_the_wallets_minimum_and_steps() {
        // Worked out with Valve's economy_common.js as it stands.
        let wallet = Wallet {
            market_minimum: 5,
            increment: 5,
            ..pounds()
        };
        assert_eq!(wallet.valid(3), 5, "at least the minimum");
        assert_eq!(wallet.valid(12), 10, "to the nearest step");
        assert_eq!(wallet.valid(13), 15, "halfway rounds up");
        assert_eq!(wallet.buyer_pays(100), 100 + 10 + 5);
        assert_eq!(
            wallet.buyer_pays(10),
            10 + 5 + 5,
            "each fee at least the minimum"
        );
        assert_eq!(wallet.seller_gets(115), 100);
        assert_eq!(wallet.seller_gets(20), 10);
        let free = Wallet {
            steam_fee: 0,
            ..pounds()
        };
        assert_eq!(free.buyer_pays(100), 110, "no Steam fee at all");
    }
}
