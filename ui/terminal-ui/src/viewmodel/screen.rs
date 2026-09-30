// What every screen is built from, as it stands this frame, and what the
// farmer is doing in the screens' terms. Pure: built fresh each frame from
// domain values, as every view model is.

use std::time::Duration;

use account::Account;
use chrono::{DateTime, FixedOffset, Utc};
use farming::{Drop, DropCard, FarmingSession, FarmingStatus, Forecast, Mode, Status};
use library::{Game, SteamLibrary};
use market::{
    Basis, Estimate, Held, HeldCard, MarketPause, Money, Price, PriceBook, Wallet, held_value,
    on_completion, value_left,
};
use preferences::Preferences;

/// Whether farming runs, as the user left it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    Running,
    /// The user paused it.
    Paused,
    /// Not started: nobody signed in yet, or before onboarding ends.
    Stopped,
}

/// Everything the screens are built from this frame: the farmer's last word,
/// the market's prices, who's signed in, the user's choices and the clock.
#[derive(Debug, Clone, Copy)]
pub struct Snapshot<'a> {
    /// The farmer's last word; `None` until it has said anything.
    pub status: Option<&'a FarmingStatus>,
    /// The library as best known: the farmer's, whose hours and drops are
    /// the latest, or the one read for browsing.
    pub library: &'a SteamLibrary,
    pub run: Run,
    pub account: Option<&'a Account>,
    pub prefs: &'a Preferences,
    /// The time to finish, as last worked out.
    pub forecast: Option<&'a Forecast>,
    pub prices: &'a PriceBook,
    /// `None` until Steam has said which currency the wallet is in.
    pub wallet: Option<Wallet>,
    pub basis: Basis,
    /// Steam's pause on price lookups, as the market last told of it.
    pub pause: Option<MarketPause>,
    pub now: DateTime<Utc>,
    /// The local clock's zone, which every time on screen is shown in.
    pub zone: FixedOffset,
}

impl<'a> Snapshot<'a> {
    pub fn library(&self) -> &'a SteamLibrary {
        self.library
    }

    pub fn session(&self) -> Option<&'a FarmingSession> {
        self.status.map(|s| &s.session)
    }

    /// The farm order: the games to be farmed, by app ID, in turn.
    pub fn order(&self) -> &'a [u32] {
        self.status.map_or(&[], |s| s.order.as_slice())
    }

    /// What's being played now; nothing while paused.
    pub fn playing(&self) -> &'a [u32] {
        match (self.status, self.run) {
            (Some(s), Run::Running) => &s.playing,
            _ => &[],
        }
    }

    /// How what's played is farmed; `None` when nothing is.
    pub fn mode(&self) -> Option<Mode> {
        self.status
            .filter(|_| self.run == Run::Running)
            .and_then(|s| s.mode)
    }

    /// This session's drops, oldest first.
    pub fn drops(&self) -> &'a [Drop] {
        self.session().map_or(&[], |s| s.drops.as_slice())
    }

    /// A game's name, or its app ID when the library doesn't have it.
    pub fn name(&self, app_id: u32) -> String {
        self.library()
            .game(app_id)
            .map_or_else(|| format!("App {app_id}"), |g| g.name.clone())
    }

    /// The game farmed alone now.
    pub fn farming_alone(&self) -> Option<&'a Game> {
        match self.mode() {
            Some(Mode::Cards) => self
                .playing()
                .first()
                .and_then(|&id| self.library().game(id)),
            _ => None,
        }
    }

    /// How long until a game's last card should drop, from the forecast.
    /// It's worked out afresh every minute; meanwhile it counts down while
    /// farming runs, and holds still while it doesn't, as farming time does.
    pub fn done_in(&self, app_id: u32) -> Option<Duration> {
        let f = self.forecast?;
        let (_, d) = f.per_game.iter().find(|(id, _)| *id == app_id)?;
        if !Activity::of(self).runs() {
            return Some(*d);
        }
        let since = (self.now - f.made_at).to_std().unwrap_or_default();
        Some(d.saturating_sub(since))
    }
}

/// What the farmer is doing, as the screens tell it.
#[derive(Debug, Clone, PartialEq)]
pub enum Activity {
    /// Nobody is signed in.
    SignedOut,
    /// Steam no longer takes the saved sign-in: farming stopped then.
    Expired { stopped_at: Option<DateTime<Utc>> },
    /// The user paused farming.
    Paused,
    /// Farming hasn't started.
    Stopped,
    /// Reading the badges for the first time: nothing is known yet.
    Reading,
    /// Reading the badges again, between games.
    Checking,
    /// One game played on its own, so its cards drop.
    Farming { app_id: u32, name: String },
    /// Games played together until the first has 3 hours; the lead first.
    BuildingHours { app_ids: Vec<u32> },
    /// Another device plays on the account, since then: farming waits.
    Waiting {
        by: Option<String>,
        since: Option<DateTime<Utc>>,
    },
    /// The connection to Steam went, for this reason; tried again then.
    Reconnecting {
        why: String,
        retry_at: Option<DateTime<Utc>>,
    },
    /// The badges couldn't be read; read again then.
    Unreadable { retry_at: Option<DateTime<Utc>> },
    /// Another session took over, so farming stopped rather than fight it.
    TakenOver,
    /// Nothing left to farm, and why; looked at again then.
    NothingToFarm {
        why: String,
        next_look: Option<DateTime<Utc>>,
    },
}

/// The farmer's notes for its errors, as it words them: the one error that
/// isn't a lost connection says so.
const UNREADABLE: &str = "couldn't read your badges";
const TAKEN_OVER: &str = "stopped:";

impl Activity {
    pub fn of(s: &Snapshot<'_>) -> Self {
        let Some(account) = s.account else {
            return Self::SignedOut;
        };
        let session = s.session();
        if account.expired {
            return Self::Expired {
                stopped_at: session.and_then(stopped_at),
            };
        }
        match s.run {
            Run::Paused => return Self::Paused,
            Run::Stopped => return Self::Stopped,
            Run::Running => {}
        }
        let Some(status) = s.status else {
            return Self::Reading;
        };
        match status.status {
            Status::Checking if s.library.is_empty() => Self::Reading,
            Status::Checking => Self::Checking,
            Status::Farming => match (status.mode, status.playing.first()) {
                (Some(Mode::Cards), Some(&app_id)) => Self::Farming {
                    app_id,
                    name: s.name(app_id),
                },
                (Some(Mode::Hours), Some(_)) => Self::BuildingHours {
                    app_ids: status.playing.clone(),
                },
                _ => Self::Checking,
            },
            Status::Blocked => Self::Waiting {
                by: status.blocked_by.map(|id| s.name(id)),
                since: session.and_then(stopped_at),
            },
            Status::Error if status.note.starts_with(UNREADABLE) => Self::Unreadable {
                retry_at: status.next_look,
            },
            Status::Error if status.note.starts_with(TAKEN_OVER) => Self::TakenOver,
            Status::Error => Self::Reconnecting {
                why: status.note.clone(),
                retry_at: status.next_look,
            },
            Status::Idle => Self::NothingToFarm {
                why: status.note.clone(),
                next_look: status.next_look,
            },
        }
    }

    /// Farming goes on: a game or a group is played, or the badges are read.
    pub fn runs(&self) -> bool {
        matches!(
            self,
            Self::Reading | Self::Checking | Self::Farming { .. } | Self::BuildingHours { .. }
        )
    }

    /// Farming time stands still, so the time to finish holds and says "of
    /// farming left", with no date.
    pub fn holds_still(&self) -> bool {
        matches!(
            self,
            Self::Expired { .. }
                | Self::Paused
                | Self::Stopped
                | Self::Waiting { .. }
                | Self::Reconnecting { .. }
                | Self::Unreadable { .. }
                | Self::TakenOver
        )
    }
}

/// When play last stopped: the end of the last stretch.
fn stopped_at(session: &FarmingSession) -> Option<DateTime<Utc>> {
    session.stretches.last().and_then(|s| s.to)
}

/// The header: the farming state and for how long, the account, how
/// friends see it, and whether the computer is kept awake.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub activity: Activity,
    /// When this session started, while farming runs: "for 8h 17m · since
    /// 09:14".
    pub since: Option<DateTime<Utc>>,
    /// The account's name, when someone is signed in: empty when Steam
    /// didn't say it.
    pub account: Option<String>,
    pub expired: bool,
    pub appear_online: bool,
    /// Farming runs, so the computer is kept awake while its games play.
    pub keeping_awake: bool,
    pub now: DateTime<Utc>,
    pub zone: FixedOffset,
}

impl Header {
    pub fn build(s: &Snapshot<'_>) -> Self {
        let activity = Activity::of(s);
        Self {
            since: s
                .session()
                .filter(|_| activity.runs())
                .map(|session| session.started_at),
            account: s.account.map(|a| a.name.clone()),
            expired: s.account.is_some_and(|a| a.expired),
            appear_online: s.prefs.appear_online,
            keeping_awake: activity.runs(),
            activity,
            now: s.now,
            zone: s.zone,
        }
    }
}

/// A card that dropped this session, as the market values it: the card it
/// was, or `None` while that isn't known.
pub fn held_card(drop: &Drop) -> Option<HeldCard> {
    match &drop.card {
        DropCard::Identified(asset) => Some(HeldCard::from(asset)),
        DropCard::NameOnly { name, foil } => Some(HeldCard::named(drop.app_id, name, *foil)),
        DropCard::Identifying | DropCard::Unknown => None,
    }
}

/// What this session's cards are worth on `basis`: each copy at its own
/// price, and every drop whose card isn't known counted as unpriced.
pub fn session_value(s: &Snapshot<'_>, drops: &[&Drop], basis: Basis) -> Option<Held> {
    let wallet = s.wallet?;
    let cards: Vec<HeldCard> = drops.iter().filter_map(|d| held_card(d)).collect();
    let unknown = drops.len() - cards.len();
    Some(held_value(
        &cards,
        u32::try_from(unknown).unwrap_or(u32::MAX),
        s.prices,
        basis,
        &wallet,
        s.now,
    ))
}

/// A price as a screen shows it: a value on the basis, or why there isn't
/// one (docs/design/ui.md §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    /// Worth this; `stale` with its age once over 6 hours old.
    Value {
        value: Money,
        stale: Option<Duration>,
    },
    /// Not looked up yet: "…".
    Pending,
    /// Nobody is selling it: "no market".
    NoMarket,
    /// It can't be sold: "—".
    NotMarketable,
    /// The lookup failed: "?".
    Failed,
    /// Priced in another currency: shown as it came, never counted.
    Foreign(Money),
}

impl Cell {
    /// A card's price on `basis`, in the wallet's currency.
    pub fn of(price: &Price, basis: Basis, wallet: Option<&Wallet>, now: DateTime<Utc>) -> Self {
        match price {
            Price::Pending => Self::Pending,
            Price::NoMarket => Self::NoMarket,
            Price::NotMarketable => Self::NotMarketable,
            Price::Failed { .. } => Self::Failed,
            Price::Known(quote) => {
                let Some(wallet) = wallet else {
                    return Self::Pending;
                };
                match market::value_of(price, basis, wallet) {
                    Some(value) => Self::Value {
                        value,
                        stale: quote.is_stale(now).then(|| quote.age(now)),
                    },
                    None => match quote.ask.filter(|a| a.currency != wallet.currency) {
                        Some(foreign) => Self::Foreign(foreign),
                        // Known, but not on this basis: an order book with no
                        // offers, or a search on the instant basis.
                        None => Self::NoMarket,
                    },
                }
            }
        }
    }

    /// Its value, when it counts.
    pub fn value(&self) -> Option<Money> {
        match self {
            Self::Value { value, .. } => Some(*value),
            _ => None,
        }
    }
}

/// How far the prices have got: the games to be farmed whose sets have been
/// looked up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pricing {
    pub priced: usize,
    pub of: usize,
}

/// This session's value on each basis, what's still to drop and the value on
/// completion, and the spares.
#[derive(Debug, Clone, PartialEq)]
pub struct Values {
    pub basis: Basis,
    /// This session's cards on the chosen basis.
    pub held: Held,
    /// This session's cards on each basis: list, net, instant.
    pub at_each: [(Basis, Held); 3],
    /// What's still to drop in the farm order.
    pub left: Estimate,
    /// The oldest price over 6 hours old among the sets still to drop.
    pub left_oldest: Option<Duration>,
    pub completion: Estimate,
    /// How many of this session's drops are spares, and what they're worth.
    pub spares: (u32, Held),
    pub pricing: Pricing,
}

impl Values {
    /// `None` until Steam has said which currency the wallet is in.
    pub fn build(s: &Snapshot<'_>) -> Option<Self> {
        let wallet = s.wallet?;
        let drops: Vec<&Drop> = s.drops().iter().collect();
        let at = |basis| session_value(s, &drops, basis);
        let held = at(s.basis)?;
        let at_each = [
            (Basis::List, at(Basis::List)?),
            (Basis::Net, at(Basis::Net)?),
            (Basis::Instant, at(Basis::Instant)?),
        ];
        let left = value_left(s.library(), s.order(), s.prices, s.basis, &wallet);
        let spare_drops: Vec<&Drop> = drops.iter().copied().filter(|d| d.is_spare()).collect();
        let spares = (
            u32::try_from(spare_drops.len()).unwrap_or(u32::MAX),
            session_value(s, &spare_drops, s.basis)?,
        );
        let order = s.order();
        let pricing = Pricing {
            priced: order
                .iter()
                .filter(|id| s.prices.sets.contains_key(id))
                .count(),
            of: order.len(),
        };
        let left_oldest = order
            .iter()
            .filter_map(|id| s.prices.sets.get(id))
            .filter_map(|set| {
                set.normal.iter().find_map(|c| match &c.price {
                    Price::Known(q) if q.is_stale(s.now) => Some(q.age(s.now)),
                    _ => None,
                })
            })
            .max();
        Some(Self {
            basis: s.basis,
            completion: on_completion(&held, &left),
            held,
            at_each,
            left,
            left_oldest,
            spares,
            pricing,
        })
    }

    /// Every game to be farmed is priced, so the value on completion can be
    /// told (a skipped game is never priced, and never waited for).
    pub fn all_priced(&self) -> bool {
        self.left.unpriced_games == 0 && self.pricing.priced >= self.pricing.of
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeDelta;
    use farming::Stretch;

    use super::*;
    use crate::viewmodel::fixtures;

    #[test]
    fn the_farmer_farming_one_game_is_farming_it() {
        let data = fixtures::farming_alone();
        let s = data.snapshot();
        assert_eq!(
            Activity::of(&s),
            Activity::Farming {
                app_id: fixtures::HEAVY_RAIN,
                name: "Heavy Rain".into()
            }
        );
        assert!(Activity::of(&s).runs());
        assert!(!Activity::of(&s).holds_still());
    }

    #[test]
    fn each_state_the_spec_mocks_is_told() {
        let states = [
            (fixtures::reading_badges(), "Reading"),
            (fixtures::building_hours(), "BuildingHours"),
            (fixtures::waiting_for_hades(), "Waiting"),
            (fixtures::paused(), "Paused"),
            (fixtures::sign_in_expired(), "Expired"),
            (fixtures::reconnecting(), "Reconnecting"),
            (fixtures::nothing_to_farm(), "NothingToFarm"),
        ];
        for (data, said) in states {
            let activity = Activity::of(&data.snapshot());
            assert!(format!("{activity:?}").starts_with(said), "{activity:?}");
        }
    }

    #[test]
    fn waiting_says_what_plays_elsewhere_and_since_when() {
        let data = fixtures::waiting_for_hades();
        let s = data.snapshot();
        let Activity::Waiting { by, since } = Activity::of(&s) else {
            panic!("not waiting");
        };
        assert_eq!(by.as_deref(), Some("Hades"));
        assert_eq!(since, Some(fixtures::at(17, 29)));
        assert!(Activity::of(&s).holds_still());
    }

    #[test]
    fn a_lost_connection_says_why_and_when_it_tries_again() {
        let data = fixtures::reconnecting();
        let Activity::Reconnecting { why, retry_at } = Activity::of(&data.snapshot()) else {
            panic!("not reconnecting");
        };
        assert_eq!(why, "the connection was reset");
        assert_eq!(retry_at, Some(fixtures::now() + TimeDelta::seconds(42)));
    }

    #[test]
    fn the_farmers_other_errors_are_told_apart() {
        let mut data = fixtures::reconnecting();
        data.status.note = "couldn't read your badges — trying again in 5 minutes".into();
        assert!(matches!(
            Activity::of(&data.snapshot()),
            Activity::Unreadable { .. }
        ));
        data.status.note = "stopped: another session took over".into();
        assert_eq!(Activity::of(&data.snapshot()), Activity::TakenOver);
    }

    #[test]
    fn an_expired_sign_in_says_when_farming_stopped() {
        let data = fixtures::sign_in_expired();
        assert_eq!(
            Activity::of(&data.snapshot()),
            Activity::Expired {
                stopped_at: Some(fixtures::now())
            }
        );
    }

    #[test]
    fn the_header_says_since_when_farming_runs_and_that_it_keeps_awake() {
        let data = fixtures::farming_alone();
        let h = Header::build(&data.snapshot());
        assert_eq!(h.since, Some(fixtures::at(9, 14)));
        assert_eq!(h.account.as_deref(), Some("alice"));
        assert!(!h.expired && !h.appear_online && h.keeping_awake);
        assert_eq!(h.now, fixtures::now());
        assert!(matches!(h.activity, Activity::Farming { .. }));

        let paused = fixtures::paused();
        let h = Header::build(&paused.snapshot());
        assert_eq!(h.since, None, "no time while paused");
        assert!(!h.keeping_awake, "nothing played");

        let expired = fixtures::sign_in_expired();
        assert!(Header::build(&expired.snapshot()).expired);
    }

    #[test]
    fn the_session_is_worth_at_least_its_priced_cards_on_each_basis() {
        let data = fixtures::farming_alone();
        let v = Values::build(&data.snapshot()).unwrap();
        let said = |h: &Held| crate::tui::format::held(h);
        assert_eq!(v.basis, Basis::List);
        assert_eq!(said(&v.held), "≥ £1.45 · 3 unpriced");
        assert_eq!(v.at_each[0].0, Basis::List);
        assert_eq!(said(&v.at_each[1].1), "≥ £1.14 · 3 unpriced", "after fees");
        assert_eq!(said(&v.at_each[2].1), "≥ £0.74 · 3 unpriced", "sold now");
        assert_eq!(v.at_each[2].0, Basis::Instant);
        assert_eq!(v.spares.0, 2);
        assert_eq!(crate::tui::format::money(v.spares.1.total), "£0.13");
        assert_eq!(crate::tui::format::about(v.left.value), "≈ £15.34");
        assert_eq!(crate::tui::format::about(v.completion.value), "≈ £16.79");
        assert!(v.completion.excl_foils);
        assert_eq!(v.pricing, Pricing { priced: 57, of: 57 });
        assert!(v.all_priced());
        assert_eq!(v.left_oldest, None);
        assert_eq!(v.held.oldest, None);
    }

    #[test]
    fn stale_sets_say_how_old_the_oldest_is() {
        let data = fixtures::prices_paused();
        let v = Values::build(&data.snapshot()).unwrap();
        let hours = |d: Option<Duration>| d.map(|d| d.as_secs() / 3600);
        assert_eq!(hours(v.held.oldest), Some(8));
        assert_eq!(hours(v.left_oldest), Some(8));
    }

    #[test]
    fn the_first_minutes_price_a_few_games_at_a_time() {
        let data = fixtures::first_minutes();
        let v = Values::build(&data.snapshot()).unwrap();
        assert_eq!(v.pricing, Pricing { priced: 14, of: 62 });
        assert_eq!(
            crate::tui::format::so_far(&v.left),
            "≈ £2.37 so far · 48 unpriced"
        );
        assert!(!v.all_priced(), "the value on completion waits");
    }

    #[test]
    fn without_a_wallet_nothing_is_valued() {
        let mut data = fixtures::farming_alone();
        data.wallet = None;
        assert_eq!(Values::build(&data.snapshot()), None);
    }

    #[test]
    fn a_price_is_shown_as_the_market_left_it() {
        let wallet = market::test_support::pounds();
        let now = fixtures::now();
        let listed = market::test_support::listing(6, 10, now - TimeDelta::hours(8));
        assert_eq!(
            Cell::of(&listed, Basis::List, Some(&wallet), now),
            Cell::Value {
                value: Money::new(6, wallet.currency),
                stale: Some(Duration::from_secs(8 * 3600))
            }
        );
        assert_eq!(
            Cell::of(&listed, Basis::Net, Some(&wallet), now).value(),
            Some(Money::new(4, wallet.currency))
        );
        assert_eq!(
            Cell::of(&listed, Basis::Instant, Some(&wallet), now),
            Cell::NoMarket
        );
        assert_eq!(
            Cell::of(&Price::Pending, Basis::List, Some(&wallet), now),
            Cell::Pending
        );
        assert_eq!(
            Cell::of(&Price::NotMarketable, Basis::List, Some(&wallet), now),
            Cell::NotMarketable
        );
        assert_eq!(
            Cell::of(&Price::failed(now), Basis::List, Some(&wallet), now),
            Cell::Failed
        );
        assert_eq!(Cell::of(&listed, Basis::List, None, now), Cell::Pending);
        let dollars = Wallet::new(market::Currency::USD);
        assert_eq!(
            Cell::of(&listed, Basis::List, Some(&dollars), now),
            Cell::Foreign(Money::new(6, wallet.currency))
        );
        assert_eq!(Cell::NoMarket.value(), None);
    }

    #[test]
    fn a_games_time_to_finish_holds_still_while_farming_does() {
        let mut data = fixtures::paused();
        let forecast = data.forecast.as_mut().unwrap();
        forecast.made_at = data.now - TimeDelta::minutes(5);
        let full = forecast.per_game[0].1;
        assert_eq!(data.snapshot().done_in(fixtures::HEAVY_RAIN), Some(full));
    }

    #[test]
    fn a_games_time_to_finish_counts_down_from_when_it_was_worked_out() {
        let mut data = fixtures::farming_alone();
        let made = data.now - TimeDelta::minutes(5);
        let forecast = data.forecast.as_mut().unwrap();
        forecast.made_at = made;
        let full = forecast.per_game[0].1;
        let s = data.snapshot();
        assert_eq!(
            s.done_in(fixtures::HEAVY_RAIN),
            Some(full - Duration::from_secs(300))
        );
        assert_eq!(s.done_in(1), None);
        assert_eq!(s.name(1), "App 1");
        assert_eq!(
            s.farming_alone().map(|g| g.app_id),
            Some(fixtures::HEAVY_RAIN)
        );
    }

    #[test]
    fn what_was_played_ends_where_the_last_stretch_ends() {
        let mut session = FarmingSession::default();
        assert_eq!(stopped_at(&session), None);
        session.stretches.push(Stretch {
            app_ids: vec![1],
            mode: Mode::Cards,
            from: fixtures::at(9, 0),
            to: Some(fixtures::at(10, 0)),
        });
        assert_eq!(stopped_at(&session), Some(fixtures::at(10, 0)));
    }
}
