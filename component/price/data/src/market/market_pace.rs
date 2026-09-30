use std::time::Duration;

/// How far apart market requests go, and how long Steam's pause lasts. The
/// default is the research's; tests may go faster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketPace {
    /// After a request, before a signed-in one: 5 seconds.
    pub signed_in: Duration,
    /// Up to this much more, at random, before a signed-in request: a second.
    pub jitter: Duration,
    /// After a request, before a signed-out one: 12 seconds, since signed
    /// out, the market allows about 25 of a kind in 5 minutes.
    pub signed_out: Duration,
    /// The pause after Steam turns a request down: 10 minutes.
    pub first_pause: Duration,
    /// The longest pause, however often Steam turns the one sent to see
    /// down: an hour.
    pub longest_pause: Duration,
    /// Before asking again after a server error: 30 seconds.
    pub after_server_error: Duration,
}

impl Default for MarketPace {
    fn default() -> Self {
        Self {
            signed_in: Duration::from_secs(5),
            jitter: Duration::from_secs(1),
            signed_out: Duration::from_secs(12),
            first_pause: Duration::from_secs(10 * 60),
            longest_pause: Duration::from_secs(60 * 60),
            after_server_error: Duration::from_secs(30),
        }
    }
}
