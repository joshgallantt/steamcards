//! The numbers about games that farming and its forecast both run on, each
//! with where it comes from (docs/research/steam-card-farming.md).

/// Hours a game needs on record before its cards drop, on most accounts.
/// ASF's and Steam Game Idler's default, and xPaw's 180 minutes. Valve
/// documents no such rule; it's what the farmers observe.
pub const HOURS_BEFORE_DROPS: f64 = 3.0;

/// The most games Steam counts as played at once.
pub const MOST_PLAYED_AT_ONCE: usize = 32;
