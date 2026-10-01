//! The numbers the session's forecast runs on, each with where it comes
//! from (research: market-and-session.md, section 3.1).

/// Until drops teach it otherwise, the time to finish assumes a card every
/// 30 minutes, ASF's figure: as if 2 drops had come in an hour of farming
/// alone. A few real drops outweigh it (research: market-and-session.md,
/// section 3.1).
pub(crate) const PRIOR_DROPS: f64 = 2.0;
pub(crate) const PRIOR_HOURS: f64 = 1.0;

/// The time to finish is given with the range it falls in 80% of the time:
/// 1.28 standard deviations either side, on a log scale.
pub(crate) const BAND_80: f64 = 1.28;
