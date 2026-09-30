//! The numbers farming runs on, each with where it comes from. They were
//! cross-checked against ASF, Steam Game Idler, xPaw's Steam-Card-Farmer,
//! Idle Master and Steam's own pages; see
//! docs/research/steam-card-farming.md.

use std::time::Duration;

/// Hours a game needs on record before its cards drop, on most accounts.
/// ASF's and Steam Game Idler's default, and xPaw's 180 minutes. Valve
/// documents no such rule; it's what the farmers observe.
pub(crate) const HOURS_BEFORE_DROPS: f64 = 3.0;

/// The most games Steam counts as played at once.
pub(crate) const MOST_AT_ONCE: usize = 32;

/// How often a game being farmed has its cards looked at: ASF's
/// `FarmingDelay` of 15 minutes, plus its 15 seconds for Steam's clock.
pub(crate) const LOOK_EVERY: Duration = Duration::from_secs(15 * 60 + 15);

/// With one drop left, sooner: Idle Master's 5 minutes.
pub(crate) const LOOK_EVERY_LAST: Duration = Duration::from_secs(5 * 60);

/// A card can take a moment to show on the card page after Steam says it
/// arrived: ASF waits 2 seconds.
pub(crate) const AFTER_NEW_ITEMS: Duration = Duration::from_secs(2);

/// A game that drops nothing for this long goes behind the others: ASF's
/// `MaxFarmingTime`.
pub(crate) const GIVE_UP_AFTER: Duration = Duration::from_secs(10 * 60 * 60);

/// A game put behind the others this often is left alone for the rest of
/// the session: Steam won't drop its cards (family-shared, free-to-play,
/// private).
pub(crate) const GIVE_UP_TIMES: u8 = 2;

/// After another device stops playing, how long before playing again, so as
/// not to take over from it: ASF's `MinFarmingDelayAfterBlock`, and Steam
/// Game Idler's 60 seconds.
pub(crate) const AFTER_BLOCK: Duration = Duration::from_secs(60);

/// With nothing to farm, how often to look at the library again: ASF's
/// `IdleFarmingPeriod`.
pub(crate) const IDLE_LOOK: Duration = Duration::from_secs(8 * 60 * 60);

/// After the library couldn't be read, when to try again. A page that
/// didn't load says nothing about the cards: it's never "none left".
pub(crate) const RETRY_READ: Duration = Duration::from_secs(5 * 60);

/// After losing touch with Steam, when to try again.
pub(crate) const RETRY_CONNECT: Duration = Duration::from_secs(60);

/// How often the farmer looks at the preferences while playing, so a change
/// shows within moments.
pub(crate) const TICK: Duration = Duration::from_secs(30);

/// Until drops teach it otherwise, the time to finish assumes a card every
/// 30 minutes, ASF's figure: as if 2 drops had come in an hour of farming
/// alone. A few real drops outweigh it (research: market-and-session.md,
/// section 3.1).
pub(crate) const PRIOR_DROPS: f64 = 2.0;
pub(crate) const PRIOR_HOURS: f64 = 1.0;

/// The time to finish is given with the range it falls in 80% of the time:
/// 1.28 standard deviations either side, on a log scale.
pub(crate) const BAND_80: f64 = 1.28;

/// Sale-event badges: earned by taking part in a sale, not by playing, so
/// playing never drops their cards. ASF's `SalesBlacklist`.
pub(crate) const SALE_EVENTS: [u32; 25] = [
    267_420, 303_700, 335_590, 368_020, 425_280, 480_730, 566_020, 639_900, 762_800, 876_740,
    991_980, 1_195_670, 1_343_890, 1_465_680, 1_658_760, 1_797_760, 2_021_850, 2_243_720,
    2_459_330, 2_640_280, 2_861_690, 2_861_720, 3_558_920, 3_558_940, 4_761_370,
];
