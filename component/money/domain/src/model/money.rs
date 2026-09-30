use std::fmt;

use crate::Currency;

/// An amount of money in one currency, in hundredths of its unit, as Steam
/// counts it: 145 pounds is £1.45. A currency written in whole units still
/// counts in hundredths, so 12300 yen is ¥ 123.
///
/// Amounts in different currencies are never added or converted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Money {
    pub minor: i64,
    pub currency: Currency,
}

impl Money {
    pub fn new(minor: i64, currency: Currency) -> Self {
        Self { minor, currency }
    }

    /// Nothing, in `currency`.
    pub fn zero(currency: Currency) -> Self {
        Self::new(0, currency)
    }

    /// The two together, or `None` when they're in different currencies,
    /// which are never added, or the sum is out of range.
    pub fn checked_add(self, other: Money) -> Option<Money> {
        if self.currency != other.currency {
            return None;
        }
        Some(Self::new(
            self.minor.checked_add(other.minor)?,
            self.currency,
        ))
    }

    /// `n` of this amount, or `None` when that's out of range.
    pub fn times(self, n: u32) -> Option<Money> {
        Some(Self::new(
            self.minor.checked_mul(i64::from(n))?,
            self.currency,
        ))
    }

    /// Written with the currency's thousands separator as well, "£1,234.50",
    /// as the site's own pages write large amounts. Valve's
    /// `v_currencyformat`, which `to_string` follows, writes "£1234.50".
    pub fn grouped(&self) -> String {
        self.currency.write(self.minor, true)
    }
}

impl fmt::Display for Money {
    /// As Valve's `v_currencyformat` writes it: "£0.05", "0,05€", "1,--€",
    /// "¥ 123", "CHF 1.50".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.currency.write(self.minor, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(minor: i64, id: u32) -> String {
        Money::new(minor, Currency::from_id(id)).to_string()
    }

    #[test]
    fn amounts_are_written_as_valves_page_writes_them() {
        assert_eq!(written(5, 2), "£0.05");
        assert_eq!(written(145, 2), "£1.45");
        assert_eq!(
            written(123_450, 2),
            "£1234.50",
            "v_currencyformat doesn't group"
        );
        assert_eq!(
            written(7, 1),
            "$0.07",
            "no \" USD\": the country isn't known"
        );
        assert_eq!(written(150, 4), "CHF 1.50");
        assert_eq!(written(150, 6), "1,50 zł");
        assert_eq!(written(150, 7), "R$ 1,50");
        assert_eq!(written(150, 17), "1,50 TL");
        assert_eq!(written(150, 45), "1.50 kr.");
        assert_eq!(written(-150, 2), "£-1.50", "as toFixed(2) writes it");
        assert_eq!(written(-50, 2), "£-0.50");
    }

    #[test]
    fn a_whole_amount_in_euros_ends_in_dashes() {
        assert_eq!(written(5, 3), "0,05€");
        assert_eq!(written(100, 3), "1,--€");
        assert_eq!(written(0, 3), "0,--€");
        assert_eq!(written(1050, 3), "10,50€");
    }

    #[test]
    fn whole_unit_currencies_drop_their_hundredths_except_roubles() {
        assert_eq!(written(12_300, 8), "¥ 123");
        assert_eq!(
            written(12_345, 8),
            "¥ 123.45",
            "only a whole amount drops them"
        );
        assert_eq!(written(150_000, 15), "1500₫");
        assert_eq!(written(123_400, 10), "Rp 1234");
        assert_eq!(
            written(100, 5),
            "1,00 руб.",
            "Valve's page keeps roubles' ,00"
        );
        assert_eq!(written(150, 5), "1,50 руб.");
    }

    #[test]
    fn large_amounts_can_be_grouped_as_the_sites_pages_do() {
        let grouped = |minor, id| Money::new(minor, Currency::from_id(id)).grouped();
        assert_eq!(grouped(123_450, 2), "£1,234.50");
        assert_eq!(grouped(12_345_678_900, 2), "£123,456,789.00");
        assert_eq!(grouped(99_900, 2), "£999.00");
        assert_eq!(grouped(123_400, 10), "Rp 1 234");
        assert_eq!(grouped(150_000_000, 15), "1.500.000₫");
        assert_eq!(grouped(100_000, 3), "1 000,--€");
        assert_eq!(
            grouped(123_400, 5),
            "1234,00 руб.",
            "roubles have no separator"
        );
    }

    #[test]
    fn an_amount_in_a_currency_valve_doesnt_know_names_its_id() {
        assert_eq!(written(150, 48), "1.50 ECurrency 48");
    }

    #[test]
    fn money_in_different_currencies_is_never_added() {
        let pounds = Money::new(145, Currency::GBP);
        assert_eq!(
            pounds.checked_add(Money::new(5, Currency::GBP)),
            Some(Money::new(150, Currency::GBP))
        );
        assert_eq!(pounds.checked_add(Money::new(7, Currency::USD)), None);
        assert_eq!(
            Money::new(i64::MAX, Currency::GBP).checked_add(pounds),
            None
        );
        assert_eq!(pounds.times(3), Some(Money::new(435, Currency::GBP)));
        assert_eq!(Money::new(i64::MAX, Currency::GBP).times(2), None);
        assert_eq!(Money::zero(Currency::EUR), Money::new(0, Currency::EUR));
    }
}
