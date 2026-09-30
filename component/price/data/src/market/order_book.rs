use anyhow::anyhow;
use serde::Deserialize;
use steam_api::inventory::STEAM_APP;

/// A card's order book. Every amount is in hundredths of `currency`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OrderBook {
    /// `amtMinSellOrder`: the lowest listing.
    pub(crate) lowest_ask: i64,
    /// `amtMaxBuyOrder`: the best offer.
    pub(crate) highest_bid: i64,
    /// `cSellOrders`: how many are listed.
    pub(crate) sell_orders: u32,
    /// `cBuyOrders`: how many are wanted.
    pub(crate) buy_orders: u32,
    /// `eCurrency`, read every time. Signed out, the order book answers in
    /// the currency of the country asked from; signed in, it's expected to
    /// answer in the wallet's.
    pub(crate) currency: u32,
}

/// `orderbook` for one card, as Steam's own new market asks:
/// `q=Load&qp=[753,"<hash name>"]`, the query encoded as a form's is.
pub(crate) fn orderbook_path(market_hash_name: &str) -> String {
    let qp = serde_json::json!([STEAM_APP, market_hash_name]).to_string();
    let query: String = reqwest::Url::parse_with_params(
        "https://steamcommunity.com/market/orderbook",
        [("q", "Load"), ("qp", qp.as_str())],
    )
    .ok()
    .and_then(|url| url.query().map(str::to_owned))
    .unwrap_or_default();
    format!("/market/orderbook?{query}")
}

/// The header Steam's own pages send with `orderbook`, as SteamDB's
/// extension does too.
pub(crate) const QUERY_ACTION: (&str, &str) = ("X-Valve-Request-Type", "queryAction");

/// An order book's reply comes wrapped in an outer `data` since about
/// 2026-08-14 (SEE PR #331), and bare before.
#[derive(Deserialize)]
#[serde(untagged)]
enum OrderBookReply {
    Wrapped { data: OrderBookAnswer },
    Bare(OrderBookAnswer),
}

#[derive(Deserialize)]
struct OrderBookAnswer {
    success: bool,
    data: Option<OrderBookData>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderBookData {
    #[serde(default)]
    amt_min_sell_order: i64,
    #[serde(default)]
    amt_max_buy_order: i64,
    #[serde(default)]
    c_sell_orders: u32,
    #[serde(default)]
    c_buy_orders: u32,
    e_currency: Option<u32>,
}

/// Reads `orderbook`'s reply, in either shape.
pub(crate) fn read_order_book(body: &str) -> anyhow::Result<OrderBook> {
    let reply: OrderBookReply = serde_json::from_str(body)
        .map_err(|e| anyhow!("the market's order book didn't read ({e})"))?;
    let (OrderBookReply::Wrapped { data: answer } | OrderBookReply::Bare(answer)) = reply;
    // An unknown or localised hash name is `success: false`.
    let data = answer
        .data
        .filter(|_| answer.success)
        .ok_or_else(|| anyhow!("the market has no order book for that card"))?;
    Ok(OrderBook {
        lowest_ask: data.amt_min_sell_order,
        highest_bid: data.amt_max_buy_order,
        sell_orders: data.c_sell_orders,
        buy_orders: data.c_buy_orders,
        currency: data
            .e_currency
            .ok_or_else(|| anyhow!("the market's order book didn't say its currency"))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_order_book_is_asked_for_as_steams_own_pages_ask() {
        assert_eq!(
            orderbook_path("620-Chell"),
            "/market/orderbook?q=Load&qp=%5B753%2C%22620-Chell%22%5D",
            "research §1.1's live request"
        );
        assert_eq!(
            orderbook_path("220-Gordon & Alyx (Foil Trading Card)"),
            "/market/orderbook?q=Load&qp=%5B753%2C%22220-Gordon+%26+Alyx+%28Foil+Trading+Card%29%22%5D"
        );
    }

    #[test]
    fn an_order_book_reads_in_either_shape() {
        // Chell's, 2026-09-29 at 16:43Z, signed out from a GB address.
        let wrapped = r#"{"data":{"success":true,"data":{"amtMaxBuyOrder":4,"amtMinSellOrder":5,
            "eCurrency":2,"cBuyOrders":41763,"cSellOrders":4662,
            "rgCompactBuyOrders":[4,4327,3,37436],"rgCompactSellOrders":[5,2,6,3]}}}"#;
        let bare = r#"{"success":true,"data":{"amtMaxBuyOrder":4,"amtMinSellOrder":5,
            "eCurrency":2,"cBuyOrders":41763,"cSellOrders":4662}}"#;
        let chell = OrderBook {
            lowest_ask: 5,
            highest_bid: 4,
            sell_orders: 4662,
            buy_orders: 41763,
            currency: 2,
        };

        assert_eq!(read_order_book(wrapped).unwrap(), chell);
        assert_eq!(read_order_book(bare).unwrap(), chell);
        assert!(
            read_order_book(r#"{"data":{"success":false}}"#).is_err(),
            "730-SAS, a name the market doesn't know"
        );
        assert!(read_order_book(r#"{"success":false}"#).is_err());
        assert!(
            read_order_book(r#"{"success":true,"data":{"amtMinSellOrder":5}}"#).is_err(),
            "no currency"
        );
    }
}
