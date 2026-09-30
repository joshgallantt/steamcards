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

    /// Every currency in Valve's table, by id.
    pub fn every() -> impl Iterator<Item = Currency> {
        (1..=47).chain([9000, 9001]).map(Self)
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
    pub(crate) fn write(self, minor: i64, grouped: bool) -> String {
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

    #[test]
    fn every_currency_is_valves_whole_table() {
        let every: Vec<Currency> = Currency::every().collect();
        assert_eq!(every.len(), 49);
        assert!(every.iter().all(|c| c.code().is_some()));
        assert_eq!(every[1], Currency::GBP);
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
    }
}
