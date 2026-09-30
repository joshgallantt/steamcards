//! Money as Steam counts it, and writes it: an amount in hundredths of a
//! currency's unit, with the currency Steam names by its `ECurrency` id.
//! Every currency's format is Valve's own, from `g_rgCurrencyData` in
//! steamcommunity.com's global.js, and an amount is written as that page's
//! `v_currencyformat` writes it (research §1.2).

use std::fmt;

/// A currency Steam prices things in, by its `ECurrency` id. An id Valve's
/// table doesn't have is kept all the same: it's never added to or converted
/// into another currency, so it only needs writing down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Currency(u32);

impl Currency {
    pub const USD: Currency = Currency(1);
    pub const GBP: Currency = Currency(2);
    pub const EUR: Currency = Currency(3);
    /// Written in whole units, but Valve's page leaves it its ",00".
    pub const RUB: Currency = Currency(5);

    /// The currency Steam calls `id`.
    pub fn from_id(id: u32) -> Self {
        Self(id)
    }

    /// Its `ECurrency` id: 1 for dollars, 2 for pounds.
    pub fn id(self) -> u32 {
        self.0
    }

    /// Its three-letter code, "GBP"; `None` for an id Valve's table doesn't
    /// have.
    pub fn code(self) -> Option<&'static str> {
        style(self.0).map(|s| s.code)
    }

    /// Its symbol as Valve writes it, "£"; `None` for an id Valve's table
    /// doesn't have.
    pub fn symbol(self) -> Option<&'static str> {
        style(self.0).map(|s| s.symbol)
    }

    /// `minor` hundredths written as Valve's `v_currencyformat` writes them,
    /// optionally with the currency's thousands separator.
    ///
    /// Valve's page adds " USD" to dollars shown outside the US. That's left
    /// out: which country the account is in isn't known here, and the
    /// market's own `sell_price_text` came without it from a GB address.
    fn write(self, minor: i64, grouped: bool) -> String {
        let sign = if minor < 0 { "-" } else { "" };
        let units = (minor / 100).unsigned_abs();
        let hundredths = (minor % 100).unsigned_abs();
        let Some(style) = style(self.0) else {
            // Valve's fallback for a currency it doesn't know: the number,
            // then the currency.
            return format!("{sign}{units}.{hundredths:02} {self}");
        };
        let units = if grouped {
            group(units, style.thousands)
        } else {
            units.to_string()
        };
        let number = if hundredths == 0 && style.whole_units && self != Self::RUB {
            format!("{sign}{units}")
        } else if hundredths == 0 && self == Self::EUR {
            // "1,--€": the one currency whose whole amounts are written so.
            format!("{sign}{units}{}--", style.decimal)
        } else {
            format!("{sign}{units}{}{hundredths:02}", style.decimal)
        };
        if style.before {
            format!("{}{}{number}", style.symbol, style.spacer)
        } else {
            format!("{number}{}{}", style.spacer, style.symbol)
        }
    }
}

impl fmt::Display for Currency {
    /// "GBP", or "ECurrency 48" for an id Valve's table doesn't have.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.code() {
            Some(code) => write!(f, "{code}"),
            None => write!(f, "ECurrency {}", self.0),
        }
    }
}

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

/// How Valve writes amounts in one currency: an entry of `g_rgCurrencyData`.
#[derive(Debug, Clone, Copy)]
struct Style {
    code: &'static str,
    symbol: &'static str,
    /// The symbol goes before the amount ("£0.05"), or after ("0,05€").
    before: bool,
    /// Amounts are written in whole units: "¥ 123", not "¥ 123.00".
    whole_units: bool,
    decimal: &'static str,
    thousands: &'static str,
    /// What goes between the symbol and the amount: "CHF 1.50", "1,50 zł".
    spacer: &'static str,
}

const BEFORE: bool = true;
const AFTER: bool = false;
const WHOLE: bool = true;
const CENTS: bool = false;

/// `g_rgCurrencyData`, from steamcommunity.com's global.js (SteamTracking,
/// 2026-07-22), by `ECurrency` id: the code, symbol, where the symbol goes,
/// whole units or not, the decimal symbol, the thousands separator, and
/// what goes between the symbol and the amount.
fn style(id: u32) -> Option<Style> {
    let (code, symbol, before, whole_units, decimal, thousands, spacer) = match id {
        1 => ("USD", "$", BEFORE, CENTS, ".", ",", ""),
        2 => ("GBP", "£", BEFORE, CENTS, ".", ",", ""),
        3 => ("EUR", "€", AFTER, CENTS, ",", " ", ""),
        4 => ("CHF", "CHF", BEFORE, CENTS, ".", " ", " "),
        5 => ("RUB", "руб.", AFTER, WHOLE, ",", "", " "),
        6 => ("PLN", "zł", AFTER, CENTS, ",", " ", " "),
        7 => ("BRL", "R$", BEFORE, CENTS, ",", ".", " "),
        8 => ("JPY", "¥", BEFORE, WHOLE, ".", ",", " "),
        9 => ("NOK", "kr", AFTER, CENTS, ",", ".", " "),
        10 => ("IDR", "Rp", BEFORE, WHOLE, ".", " ", " "),
        11 => ("MYR", "RM", BEFORE, CENTS, ".", ",", ""),
        12 => ("PHP", "P", BEFORE, CENTS, ".", ",", ""),
        13 => ("SGD", "S$", BEFORE, CENTS, ".", ",", ""),
        14 => ("THB", "฿", BEFORE, CENTS, ".", ",", ""),
        15 => ("VND", "₫", AFTER, WHOLE, ",", ".", ""),
        16 => ("KRW", "₩", BEFORE, WHOLE, ".", ",", " "),
        17 => ("TRY", "TL", AFTER, CENTS, ",", ".", " "),
        18 => ("UAH", "₴", AFTER, WHOLE, ",", " ", ""),
        19 => ("MXN", "Mex$", BEFORE, CENTS, ".", ",", " "),
        20 => ("CAD", "CDN$", BEFORE, CENTS, ".", ",", " "),
        21 => ("AUD", "A$", BEFORE, CENTS, ".", ",", " "),
        22 => ("NZD", "NZ$", BEFORE, CENTS, ".", ",", " "),
        23 => ("CNY", "¥", BEFORE, CENTS, ".", ",", " "),
        24 => ("INR", "₹", BEFORE, WHOLE, ".", ",", " "),
        25 => ("CLP", "CLP$", BEFORE, WHOLE, ",", ".", " "),
        26 => ("PEN", "S/.", BEFORE, CENTS, ".", ",", ""),
        27 => ("COP", "COL$", BEFORE, WHOLE, ",", ".", " "),
        28 => ("ZAR", "R", BEFORE, CENTS, ".", " ", " "),
        29 => ("HKD", "HK$", BEFORE, CENTS, ".", ",", " "),
        30 => ("TWD", "NT$", BEFORE, WHOLE, ".", ",", " "),
        31 => ("SAR", "SR", AFTER, CENTS, ".", ",", " "),
        32 => ("AED", "AED", AFTER, CENTS, ".", ",", " "),
        33 => ("SEK", "kr", AFTER, CENTS, ".", ",", " "),
        34 => ("ARS", "ARS$", BEFORE, CENTS, ",", ".", " "),
        35 => ("ILS", "₪", BEFORE, CENTS, ".", ",", ""),
        36 => ("BYN", "Br", BEFORE, CENTS, ".", ",", ""),
        37 => ("KZT", "₸", AFTER, WHOLE, ",", " ", ""),
        38 => ("KWD", "KD", AFTER, CENTS, ".", ",", " "),
        39 => ("QAR", "QR", AFTER, CENTS, ".", ",", " "),
        40 => ("CRC", "₡", BEFORE, WHOLE, ",", ".", ""),
        41 => ("UYU", "$U", BEFORE, WHOLE, ",", ".", ""),
        42 => ("BGN", "лв", AFTER, CENTS, ".", ",", " "),
        43 => ("HRK", "kn", AFTER, CENTS, ".", ",", " "),
        44 => ("CZK", "Kč", AFTER, CENTS, ".", ",", " "),
        45 => ("DKK", "kr.", AFTER, CENTS, ".", ",", " "),
        46 => ("HUF", "Ft", AFTER, CENTS, ".", ",", " "),
        47 => ("RON", "lei", AFTER, CENTS, ".", ",", " "),
        9000 => ("RMB", "刀币", AFTER, WHOLE, ".", "", " "),
        9001 => ("NXP", "원", AFTER, WHOLE, ".", ",", ""),
        _ => return None,
    };
    Some(Style {
        code,
        symbol,
        before,
        whole_units,
        decimal,
        thousands,
        spacer,
    })
}

/// `n` with `separator` between each group of three digits: 1234567 with
/// "," is "1,234,567".
fn group(n: u64, separator: &str) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() * 2);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push_str(separator);
        }
        out.push(digit);
    }
    out
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
    fn a_currency_is_known_by_its_code_and_symbol() {
        assert_eq!(Currency::GBP.code(), Some("GBP"));
        assert_eq!(Currency::GBP.symbol(), Some("£"));
        assert_eq!(Currency::from_id(23).symbol(), Some("¥"), "yuan");
        assert_eq!(Currency::from_id(47).to_string(), "RON");
        assert_eq!(Currency::from_id(47).id(), 47);
        assert_eq!(Currency::from_id(48).code(), None);
        assert_eq!(Currency::from_id(48).to_string(), "ECurrency 48");
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
