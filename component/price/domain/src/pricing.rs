//! Pricing a game's set: looking it up afresh, normal cards then foils, and
//! keeping what's found.

use card::CardKind;
use steam_library::AppId;

use crate::{
    Clock, Lookup, MarketPause, PriceRepository, SetPrices,
    rules::{RETRY_FAILED, later},
};

/// How pricing a set went.
pub(crate) enum Priced {
    Done,
    Paused(MarketPause),
    /// Steam's answer couldn't be used, for this reason.
    Failed(String),
    /// The market couldn't be asked, or didn't answer, for this reason:
    /// nothing was kept.
    Unanswered(String),
}

/// Looks a game's set up afresh, normal cards then foils, and keeps it. A
/// set's prices come from one lookup at one time: when either half's answer
/// can't be used, it keeps the prices it had, and is tried again a day
/// later. When the market couldn't be asked, it's left as it was.
pub(crate) async fn price_set(repo: &dyn PriceRepository, app_id: AppId, clock: &Clock) -> Priced {
    let mut kinds = [Vec::new(), Vec::new()];
    for (kind, cards) in [CardKind::Normal, CardKind::Foil]
        .into_iter()
        .zip(&mut kinds)
    {
        match repo.look_up_set(app_id, kind).await {
            Ok(Lookup::Found(found)) => *cards = found,
            Ok(Lookup::Paused(pause)) => return Priced::Paused(pause),
            Ok(Lookup::Unanswered(why)) => return Priced::Unanswered(why),
            Err(e) => {
                let now = clock();
                let failed = match repo.book().sets.get(&app_id) {
                    Some(before) => SetPrices {
                        retry_at: Some(later(now, RETRY_FAILED)),
                        ..before.clone()
                    },
                    None => SetPrices::failed(app_id, now),
                };
                repo.keep_set(failed);
                return Priced::Failed(e.to_string());
            }
        }
    }
    let [normal, foil] = kinds;
    repo.keep_set(SetPrices {
        app_id,
        normal,
        foil,
        fetched_at: clock(),
        retry_at: None,
    });
    Priced::Done
}
