//! The numbers farming runs on, each with where it comes from. They were
//! cross-checked against ASF, Steam Game Idler, xPaw's Steam-Card-Farmer,
//! Idle Master and Steam's own pages; see
//! docs/research/steam-card-farming.md.

use std::time::Duration;

use steam_library::AppId;

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

/// After another device takes over playing, how long to wait for Steam to
/// say it's playing before playing again: its game can take minutes to start
/// (an update, or shaders to prepare), and playing meanwhile would have its
/// Steam client ask to take over again.
pub(crate) const AFTER_TAKEN_OVER: Duration = Duration::from_secs(5 * 60);

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

/// Sale-event badges: earned by taking part in a sale, not by playing, so
/// playing never drops their cards. ASF's `SalesBlacklist`.
pub(crate) const SALE_EVENTS: [AppId; 25] = [
    AppId(267_420),
    AppId(303_700),
    AppId(335_590),
    AppId(368_020),
    AppId(425_280),
    AppId(480_730),
    AppId(566_020),
    AppId(639_900),
    AppId(762_800),
    AppId(876_740),
    AppId(991_980),
    AppId(1_195_670),
    AppId(1_343_890),
    AppId(1_465_680),
    AppId(1_658_760),
    AppId(1_797_760),
    AppId(2_021_850),
    AppId(2_243_720),
    AppId(2_459_330),
    AppId(2_640_280),
    AppId(2_861_690),
    AppId(2_861_720),
    AppId(3_558_920),
    AppId(3_558_940),
    AppId(4_761_370),
];
