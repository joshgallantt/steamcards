// The spec's shared data set as domain values (docs/design/ui.md, "The data
// set"), and a function for each state its mockups show. Test-only: the view
// models' tests read it, and the screens' fixtures build an App from it.
//
// Tuesday 29 September 2026. The session ran from 09:14 to 17:31: 16 drops,
// five games finished, Heavy Rain farmed now. Times are on a fixed local
// clock an hour ahead of UTC, so a screen that showed UTC would read wrong.
// The library's prices and the queued games past the spec's are invented, as
// the spec's are: each set's cards average the drop value the mockups show.

use std::time::Duration;

use account::Account;
use chrono::{DateTime, FixedOffset, TimeDelta, TimeZone, Utc};
use farming::{
    Drop, DropCard, FarmingSession, FarmingStatus, Finished, Forecast, Mode, SetAside, Status,
    Stretch,
};
use library::{Card, CardAsset, CardDrops, Game, SteamLibrary};
use market::{
    Basis, MarketPause, Offers, Price, PriceBook, PricedCard, SetPrices, Wallet,
    test_support::{listing, order_book, pounds, priced_card},
};
use preferences::Preferences;

use super::screen::{Run, Snapshot};

pub(crate) const HEAVY_RAIN: u32 = 960_910;
pub(crate) const LIMBO: u32 = 48_000;
pub(crate) const STRAY: u32 = 1_332_010;
pub(crate) const WARFRAME: u32 = 230_410;
pub(crate) const HOLLOW_KNIGHT: u32 = 367_520;
pub(crate) const INSCRYPTION: u32 = 1_092_790;
pub(crate) const HADES: u32 = 1_145_360;
pub(crate) const CELESTE: u32 = 504_230;
pub(crate) const GOROGOA: u32 = 557_600;
pub(crate) const COUNTER_STRIKE: u32 = 730;
pub(crate) const VAMPIRE_SURVIVORS: u32 = 1_794_680;
pub(crate) const CULT_OF_THE_LAMB: u32 = 1_313_140;

/// The local clock: British Summer Time.
pub(crate) fn zone() -> FixedOffset {
    FixedOffset::east_opt(3600).expect("an hour east")
}

/// `h`:`m` on Tuesday 29 September 2026, on the local clock.
pub(crate) fn at(h: u32, m: u32) -> DateTime<Utc> {
    on(0, h, m)
}

/// `h`:`m`, `days` after Tuesday 29 September 2026, on the local clock.
pub(crate) fn on(days: u32, h: u32, m: u32) -> DateTime<Utc> {
    zone()
        .with_ymd_and_hms(2026, 9, 29, h, m, 0)
        .single()
        .expect("a valid time")
        .to_utc()
        + TimeDelta::days(i64::from(days))
}

/// 17:31, when every mockup but the first minutes' and the last's is drawn.
pub(crate) fn now() -> DateTime<Utc> {
    at(17, 31)
}

/// A time on the clock: hours and minutes.
type Clock = (u32, u32);

/// A time on a day of the session: the day, counted from Tuesday, then
/// hours and minutes.
type Day = (u32, u32, u32);

/// The farm queue in the farmer's order: name, app ID, hours, drops
/// received and in all, and what a drop is worth at list prices, in pence.
/// Those past Dead by Daylight are invented, at 0.0h; Warframe, set aside
/// once, goes last.
const QUEUE: [(&str, u32, f64, u32, u32, i64); 57] = [
    ("Heavy Rain", HEAVY_RAIN, 4.0, 3, 4, 5),
    ("LIMBO", LIMBO, 3.4, 3, 5, 6),
    ("Oxygen Not Included", 457_140, 3.4, 3, 5, 7),
    ("Anno 1800", 916_440, 3.5, 5, 8, 9),
    ("Crypt of the NecroDancer", 247_080, 3.4, 4, 7, 5),
    ("Kerbal Space Program", 220_200, 3.4, 4, 7, 6),
    ("Outlast 2", 414_700, 3.4, 4, 7, 5),
    ("People Playground", 1_118_200, 3.4, 4, 7, 4),
    (
        "We Were Here Expeditions: The FriendShip",
        2_101_340,
        8.7,
        3,
        6,
        4,
    ),
    ("Desperados III", 610_370, 3.4, 5, 9, 7),
    ("Graveyard Keeper", 599_140, 3.4, 5, 9, 5),
    ("MONSTER HUNTER RISE", 1_446_780, 3.4, 5, 9, 8),
    ("Halls of Torment", 2_218_750, 3.4, 6, 11, 4),
    ("Cult of the Lamb", CULT_OF_THE_LAMB, 3.4, 7, 13, 6),
    ("Days Gone", 1_259_420, 3.4, 7, 13, 9),
    ("Little Nightmares II", 860_510, 3.4, 8, 15, 6),
    ("Monster Train", 1_102_190, 3.4, 8, 15, 5),
    ("TCG Card Shop Simulator", 3_070_070, 2.2, 4, 6, 4),
    ("FOR HONOR", 304_390, 2.1, 6, 11, 5),
    (
        "Warhammer 40,000: Dawn of War II - Anniversary Edition",
        15_620,
        1.9,
        6,
        7,
        8,
    ),
    ("Against the Storm", 1_336_490, 1.6, 6, 9, 6),
    ("Stray", STRAY, 1.4, 3, 4, 14),
    ("Total War: ATTILA", 325_610, 0.7, 3, 6, 7),
    ("Undertale", 391_540, 0.7, 3, 6, 12),
    ("Palworld", 1_623_730, 0.4, 5, 9, 9),
    ("Papers, Please", 239_030, 0.4, 4, 7, 8),
    ("Witch It", 559_650, 0.4, 5, 9, 3),
    ("The Binding of Isaac: Rebirth", 250_900, 0.3, 7, 13, 7),
    ("FATE", 246_840, 0.2, 3, 5, 5),
    ("Baldur's Gate 3", 1_086_940, 0.0, 6, 12, 13),
    ("Cities: Skylines", 255_710, 0.0, 3, 6, 6),
    ("Danganronpa 2: Goodbye Despair", 413_420, 0.0, 5, 10, 8),
    ("Danganronpa: Trigger Happy Havoc", 413_410, 0.0, 5, 10, 7),
    ("Dead by Daylight", 381_210, 0.0, 4, 8, 4),
    ("Deep Rock Galactic", 548_430, 0.0, 0, 5, 7),
    ("Disco Elysium", 632_470, 0.0, 0, 4, 9),
    ("Dishonored 2", 403_640, 0.0, 0, 5, 7),
    ("Dorfromantik", 1_455_840, 0.0, 0, 4, 5),
    ("DREDGE", 1_562_430, 0.0, 0, 5, 6),
    ("Enter the Gungeon", 311_690, 0.0, 0, 5, 5),
    ("Factorio", 427_520, 0.0, 0, 5, 9),
    ("Frostpunk", 323_190, 0.0, 0, 5, 7),
    ("Hotline Miami", 219_150, 0.0, 0, 4, 5),
    ("INSIDE", 304_430, 0.0, 0, 5, 6),
    ("Katana ZERO", 460_950, 0.0, 0, 4, 6),
    ("Loop Hero", 1_282_730, 0.0, 0, 5, 5),
    ("Northgard", 466_560, 0.0, 0, 5, 6),
    ("Oddworld: Soulstorm", 844_260, 0.0, 0, 5, 6),
    ("Pentiment", 1_205_520, 0.0, 0, 5, 7),
    ("Raft", 648_800, 0.0, 0, 5, 5),
    ("Return of the Obra Dinn", 653_530, 0.0, 0, 5, 9),
    ("Slay the Spire", 646_570, 0.0, 0, 5, 6),
    ("Spiritfarer", 972_660, 0.0, 0, 5, 7),
    ("Terraria", 105_600, 0.0, 0, 5, 7),
    ("Valheim", 892_970, 0.0, 0, 5, 6),
    ("Vampire Survivors", VAMPIRE_SURVIVORS, 0.0, 0, 5, 5),
    ("Warframe", WARFRAME, 16.2, 2, 6, 4),
];

/// The games finished this session: name, app ID, hours now, drops, drops
/// received before the session, and when its last card dropped.
const DONE: [(&str, u32, f64, u32, u32, Clock); 5] = [
    ("Hollow Knight", HOLLOW_KNIGHT, 7.1, 4, 1, (10, 41)),
    ("Inscryption", INSCRYPTION, 7.9, 4, 1, (12, 20)),
    ("Hades", HADES, 10.1, 4, 0, (14, 35)),
    ("Celeste", CELESTE, 8.0, 4, 2, (15, 41)),
    ("Gorogoa", GOROGOA, 6.2, 3, 1, (16, 48)),
];

/// Where each game farmed this session began: name, app ID, hours and drops
/// received at 09:14. The rest of the start's queue is `QUEUE` as it was.
const STARTED_WITH: [(&str, u32, f64, u32, u32, i64); 6] = [
    ("Hollow Knight", HOLLOW_KNIGHT, 5.6, 1, 4, 9),
    ("Inscryption", INSCRYPTION, 6.2, 1, 4, 6),
    ("Hades", HADES, 7.8, 0, 4, 8),
    ("Celeste", CELESTE, 6.9, 2, 4, 6),
    ("Gorogoa", GOROGOA, 5.1, 1, 3, 4),
    ("Heavy Rain", HEAVY_RAIN, 3.4, 1, 4, 5),
];

/// A drop of the session's: when, which game, the card, whether a foil, and
/// which copy.
type Dropped = (Clock, u32, Option<&'static str>, bool, Option<u32>);

/// This session's drops: when, which game, the card, whether a foil, and
/// which copy. `None` for the card still being found out at 17:23.
const DROPS: [Dropped; 16] = [
    ((9, 44), HOLLOW_KNIGHT, Some("Hornet"), false, Some(1)),
    ((10, 12), HOLLOW_KNIGHT, Some("Zote"), false, Some(1)),
    ((10, 41), HOLLOW_KNIGHT, Some("The Knight"), false, Some(1)),
    ((11, 15), INSCRYPTION, Some("Leshy"), false, Some(1)),
    ((11, 43), INSCRYPTION, Some("Stoat"), false, Some(1)),
    ((12, 20), INSCRYPTION, Some("Stinkbug"), false, Some(1)),
    ((12, 58), HADES, Some("Zagreus"), false, Some(1)),
    ((13, 30), HADES, Some("Zagreus"), false, Some(2)),
    ((14, 2), HADES, Some("Thanatos"), true, Some(1)),
    ((14, 35), HADES, Some("Nyx"), false, Some(1)),
    ((15, 9), CELESTE, Some("Madeline"), false, Some(1)),
    ((15, 41), CELESTE, Some("Badeline"), false, Some(1)),
    ((16, 15), GOROGOA, Some("The Boy"), false, Some(1)),
    ((16, 48), GOROGOA, Some("The Fruit"), false, Some(1)),
    ((17, 5), HEAVY_RAIN, Some("Madison"), false, Some(2)),
    ((17, 23), HEAVY_RAIN, None, false, None),
];

/// What was played, alone, and when: 7h 40m of the 8h 17m, since the user
/// paused for half an hour at 13:31 and the farmer took a moment between
/// some games. The last goes on.
const STRETCHES: [(u32, Clock, Option<Clock>); 7] = [
    (HOLLOW_KNIGHT, (9, 14), Some((10, 44))),
    (INSCRYPTION, (10, 46), Some((12, 23))),
    (HADES, (12, 25), Some((13, 31))),
    (HADES, (14, 1), Some((14, 37))),
    (CELESTE, (14, 40), Some((15, 46))),
    (GOROGOA, (15, 46), Some((16, 53))),
    (HEAVY_RAIN, (16, 53), None),
];

/// Best offers for the cards held and Heavy Rain's set, in pence: the
/// instant basis.
const BIDS: [(u32, &str, bool, i64, i64); 16] = [
    (HOLLOW_KNIGHT, "Hornet", false, 9, 7),
    (HOLLOW_KNIGHT, "Zote", false, 7, 5),
    (HOLLOW_KNIGHT, "The Knight", false, 11, 8),
    (INSCRYPTION, "Leshy", false, 6, 4),
    (INSCRYPTION, "Stoat", false, 5, 4),
    (INSCRYPTION, "Stinkbug", false, 5, 3),
    (HADES, "Zagreus", false, 8, 6),
    (HADES, "Thanatos", true, 62, 41),
    (HADES, "Nyx", false, 9, 7),
    (CELESTE, "Badeline", false, 6, 4),
    (GOROGOA, "The Boy", false, 4, 3),
    (HEAVY_RAIN, "Ethan", false, 5, 3),
    (HEAVY_RAIN, "Carter", false, 4, 3),
    (HEAVY_RAIN, "Madison", false, 5, 4),
    (HEAVY_RAIN, "Norman", false, 6, 4),
    (HEAVY_RAIN, "Scott", false, 4, 3),
];

/// Everything a screen is built from, as one of the spec's mockups has it.
pub(crate) struct DataSet {
    pub(crate) status: FarmingStatus,
    pub(crate) run: Run,
    pub(crate) account: Option<Account>,
    pub(crate) prefs: Preferences,
    pub(crate) forecast: Option<Forecast>,
    pub(crate) prices: PriceBook,
    pub(crate) wallet: Option<Wallet>,
    pub(crate) basis: Basis,
    pub(crate) pause: Option<MarketPause>,
    /// When Steam first turned a lookup down, in the pause going on.
    pub(crate) paused_since: Option<DateTime<Utc>>,
    pub(crate) now: DateTime<Utc>,
    /// The chosen game.
    pub(crate) selected: u32,
}

impl DataSet {
    pub(crate) fn snapshot(&self) -> Snapshot<'_> {
        Snapshot {
            status: Some(&self.status),
            library: &self.status.library,
            run: self.run,
            account: self.account.as_ref(),
            prefs: &self.prefs,
            forecast: self.forecast.as_ref(),
            prices: &self.prices,
            wallet: self.wallet,
            basis: self.basis,
            pause: self.pause,
            now: self.now,
            zone: zone(),
        }
    }
}

fn game(name: &str, app_id: u32, hours: f64, received: u32, total: u32) -> Game {
    Game {
        app_id,
        name: name.to_owned(),
        hours,
        drops: CardDrops {
            received,
            remaining: total - received,
        },
        badge_level: 0,
        cards: Vec::new(),
    }
}

fn cards(owned: &[(&str, u32)]) -> Vec<Card> {
    owned
        .iter()
        .map(|&(name, owned)| Card {
            name: name.to_owned(),
            owned,
        })
        .collect()
}

/// A card's copy as Steam describes it, with its market hash name.
fn asset(asset_id: u64, app_id: u32, name: &str, foil: bool) -> CardAsset {
    CardAsset {
        asset_id,
        app_id,
        name: name.to_owned(),
        market_hash_name: priced_card(app_id, name, foil, Price::Pending).market_hash_name,
        foil,
        marketable: true,
        tradable: true,
    }
}

// ── The library ──────────────────────────────────────────────────────────────

/// Heavy Rain's set as the farmer last read it: Madison twice, Scott once.
fn heavy_rain_cards() -> Vec<Card> {
    cards(&[
        ("Ethan", 0),
        ("Carter", 0),
        ("Madison", 2),
        ("Norman", 0),
        ("Scott", 1),
    ])
}

/// The library at 17:31: the queue, the five games finished this session,
/// and Counter-Strike 2, skipped. 183 of 421 drops, 5 of 63 games done.
fn library() -> SteamLibrary {
    let mut games: Vec<Game> = QUEUE
        .iter()
        .map(|&(name, id, hours, got, all, _)| game(name, id, hours, got, all))
        .collect();
    games[0].cards = heavy_rain_cards();
    games.extend(
        DONE.iter()
            .map(|&(name, id, hours, all, _, _)| game(name, id, hours, all, all)),
    );
    games.push(game("Counter-Strike 2", COUNTER_STRIKE, 350.0, 0, 2));
    let mut library = SteamLibrary::new(games);
    for (id, set) in [
        (
            HOLLOW_KNIGHT,
            cards(&[
                ("The Knight", 2),
                ("Hornet", 1),
                ("Zote", 1),
                ("Quirrel", 0),
                ("Cornifer", 0),
                ("Iselda", 0),
                ("Grimm", 0),
            ]),
        ),
        (
            INSCRYPTION,
            cards(&[
                ("Leshy", 1),
                ("Stoat", 1),
                ("Stinkbug", 1),
                ("Grizzly", 1),
                ("Ouroboros", 0),
            ]),
        ),
        (
            HADES,
            cards(&[
                ("Zagreus", 2),
                ("Nyx", 1),
                ("Thanatos", 0),
                ("Megaera", 0),
                ("Hypnos", 0),
            ]),
        ),
        (
            CELESTE,
            cards(&[
                ("Madeline", 1),
                ("Badeline", 1),
                ("Theo", 1),
                ("Granny", 1),
                ("Oshiro", 0),
            ]),
        ),
        (
            GOROGOA,
            cards(&[
                ("The Boy", 1),
                ("The Fruit", 1),
                ("The Man", 1),
                ("The Bowl", 0),
            ]),
        ),
    ] {
        if let Some(mut g) = library.game(id).cloned() {
            g.cards = set;
            library.update(g);
        }
    }
    library
}

/// The farm order at 17:31: the queue, as the farmer ranks it.
fn order() -> Vec<u32> {
    QUEUE.iter().map(|g| g.1).collect()
}

// ── The session ──────────────────────────────────────────────────────────────

fn drops() -> Vec<Drop> {
    DROPS
        .iter()
        .enumerate()
        .map(|(i, &((h, m), app_id, card, foil, copy))| Drop {
            at: at(h, m),
            app_id,
            card: card.map_or(DropCard::Identifying, |name| {
                DropCard::Identified(asset(31_001 + i as u64, app_id, name, foil))
            }),
            copy,
        })
        .collect()
}

fn stretches() -> Vec<Stretch> {
    STRETCHES
        .iter()
        .map(|&(app_id, (fh, fm), to)| Stretch {
            app_ids: vec![app_id],
            mode: Mode::Cards,
            from: at(fh, fm),
            to: to.map(|(h, m)| at(h, m)),
        })
        .collect()
}

/// The session at 17:31: 16 drops, five games finished, Heavy Rain on the
/// go since 16:53.
fn session() -> FarmingSession {
    FarmingSession {
        started_at: at(9, 14),
        drops_left_at_start: Some(252),
        games_at_start: Some(62),
        drops: drops(),
        stretches: stretches(),
        finished: DONE
            .iter()
            .map(|&(_, app_id, _, _, _, (h, m))| Finished {
                app_id,
                at: at(h, m),
            })
            .collect(),
        first_forecast: Some(Forecast {
            eta: hours(5 * 24 + 6),
            band: Some((hours(2 * 24 + 18), hours(10 * 24 + 1))),
            assumed: false,
            hours_term: hours(3),
            rate: 2.0,
            per_game: Vec::new(),
            made_at: at(10, 12),
        }),
    }
}

fn hours(h: u64) -> Duration {
    Duration::from_secs(h * 3600)
}

fn minutes(m: f64) -> Duration {
    Duration::from_secs_f64(m * 60.0)
}

// ── The forecast ─────────────────────────────────────────────────────────────

/// Drops an hour learnt from 16 in 7h 40m farming alone: 2.1.
fn rate() -> f64 {
    (2.0 + 16.0) / (1.0 + 7.0 + 40.0 / 60.0)
}

/// Minutes until each game's last card, in farm order, as the spec's
/// generator schedules them: each game's drops at its rate, plus the hours
/// term once, at the first game short of 3 hours.
fn schedule(
    games: &[(u32, f64, u32)],
    rate_of: impl Fn(usize) -> f64,
    hours_term: f64,
) -> Vec<(u32, f64)> {
    let mut t = 0.0;
    let mut added = false;
    games
        .iter()
        .enumerate()
        .map(|(i, &(id, hours, left))| {
            if hours < 3.0 && !added {
                t += hours_term * 60.0;
                added = true;
            }
            t += f64::from(left) / rate_of(i) * 60.0;
            (id, t)
        })
        .collect()
}

/// The forecast at 17:31: ≈ 4d 21h, 80% 3d 13h – 6d 15h, 2.1 drops an
/// hour, including ≈ 3h of building hours. Heavy Rain goes at its own pace:
/// 2 drops in 38 minutes.
fn forecast() -> Forecast {
    let games: Vec<(u32, f64, u32)> = QUEUE
        .iter()
        .map(|&(_, id, hours, got, all, _)| (id, hours, all - got))
        .collect();
    let heavy_rain = 4.0 / (2.0 / rate() + 38.0 / 60.0);
    let plan = schedule(&games, |i| if i == 0 { heavy_rain } else { rate() }, 3.0);
    let eta = (4.0 * 24.0 + 21.0) * 60.0;
    let scale = eta / plan.last().map_or(eta, |p| p.1);
    Forecast {
        eta: minutes(eta),
        band: Some((hours(3 * 24 + 13), hours(6 * 24 + 15))),
        assumed: false,
        hours_term: hours(3),
        rate: rate(),
        per_game: plan
            .into_iter()
            .map(|(id, m)| (id, minutes(m * scale)))
            .collect(),
        made_at: now(),
    }
}

// ── Prices ───────────────────────────────────────────────────────────────────

/// Five cards around a drop's value: from its low to its high, averaging it
/// exactly where the set allows. Foils go from 25p + 4 drops to 45p + 15.
fn around(ev: i64) -> ([i64; 5], [i64; 5]) {
    let (lo, hi) = ((ev - 2).max(3), ev + 2 + ev / 6);
    let rest = 5 * ev - lo - hi;
    let (third, over) = (rest / 3, rest % 3);
    let normal = [
        lo,
        hi,
        third + i64::from(over > 0),
        third + i64::from(over > 1),
        third,
    ];
    let (flo, fhi) = (25 + ev * 4, 45 + ev * 15);
    let mid = (flo + fhi) / 2;
    (normal, [flo, fhi, mid, mid, mid])
}

fn set(
    app_id: u32,
    normal: &[(&str, Price)],
    foil: &[(&str, Price)],
    at: DateTime<Utc>,
) -> SetPrices {
    let border = |cards: &[(&str, Price)], foil: bool| -> Vec<PricedCard> {
        cards
            .iter()
            .map(|(name, price)| priced_card(app_id, name, foil, price.clone()))
            .collect()
    };
    SetPrices {
        app_id,
        normal: border(normal, false),
        foil: border(foil, true),
        fetched_at: at,
        retry_at: None,
    }
}

/// A set whose cards are listed at these prices, looked up at `at`.
fn listed(
    app_id: u32,
    normal: &[(&str, i64)],
    foil: &[(&str, i64)],
    at: DateTime<Utc>,
) -> SetPrices {
    let known = |cards: &[(&str, i64)], foil: bool| -> Vec<PricedCard> {
        cards
            .iter()
            .map(|&(name, pence)| priced_card(app_id, name, foil, listing(pence, 20, at)))
            .collect()
    };
    SetPrices {
        app_id,
        normal: known(normal, false),
        foil: known(foil, true),
        fetched_at: at,
        retry_at: None,
    }
}

/// A queued game's set, invented around its drop's value.
fn queued_set(app_id: u32, ev: i64, at: DateTime<Utc>) -> SetPrices {
    if app_id == HEAVY_RAIN {
        return listed(
            app_id,
            &[
                ("Ethan", 5),
                ("Carter", 4),
                ("Madison", 5),
                ("Norman", 6),
                ("Scott", 4),
            ],
            &[
                ("Ethan", 42),
                ("Carter", 35),
                ("Madison", 60),
                ("Norman", 51),
                ("Scott", 38),
            ],
            at,
        );
    }
    let (normal, foil) = around(ev);
    let names = ["Card 1", "Card 2", "Card 3", "Card 4", "Card 5"];
    let named = |prices: [i64; 5]| -> Vec<(&str, i64)> { names.into_iter().zip(prices).collect() };
    listed(app_id, &named(normal), &named(foil), at)
}

/// The sets of the games finished this session, and so of the cards held.
/// Madeline's price is still on its way, and nobody is selling The Fruit.
fn finished_sets(at: impl Fn(u32) -> DateTime<Utc>) -> Vec<SetPrices> {
    let hk = at(HOLLOW_KNIGHT);
    let celeste = at(CELESTE);
    let gorogoa = at(GOROGOA);
    vec![
        listed(
            HOLLOW_KNIGHT,
            &[
                ("The Knight", 11),
                ("Hornet", 9),
                ("Zote", 7),
                ("Quirrel", 8),
                ("Cornifer", 8),
                ("Iselda", 9),
                ("Grimm", 12),
            ],
            &[
                ("The Knight", 95),
                ("Hornet", 88),
                ("Zote", 70),
                ("Quirrel", 74),
                ("Cornifer", 69),
                ("Iselda", 81),
                ("Grimm", 140),
            ],
            hk,
        ),
        listed(
            INSCRYPTION,
            &[
                ("Leshy", 6),
                ("Stoat", 5),
                ("Stinkbug", 5),
                ("Grizzly", 7),
                ("Ouroboros", 7),
            ],
            &[
                ("Leshy", 49),
                ("Stoat", 45),
                ("Stinkbug", 44),
                ("Grizzly", 52),
                ("Ouroboros", 58),
            ],
            at(INSCRYPTION),
        ),
        listed(
            HADES,
            &[
                ("Zagreus", 8),
                ("Nyx", 9),
                ("Thanatos", 8),
                ("Megaera", 7),
                ("Hypnos", 8),
            ],
            &[
                ("Zagreus", 58),
                ("Nyx", 66),
                ("Thanatos", 62),
                ("Megaera", 57),
                ("Hypnos", 60),
            ],
            at(HADES),
        ),
        set(
            CELESTE,
            &[
                ("Madeline", Price::Pending),
                ("Badeline", listing(6, 20, celeste)),
                ("Theo", listing(6, 20, celeste)),
                ("Granny", listing(6, 20, celeste)),
                ("Oshiro", listing(6, 20, celeste)),
            ],
            &[
                ("Madeline", listing(49, 5, celeste)),
                ("Badeline", listing(52, 5, celeste)),
            ],
            celeste,
        ),
        set(
            GOROGOA,
            &[
                ("The Boy", listing(4, 20, gorogoa)),
                ("The Fruit", Price::NoMarket),
                ("The Man", listing(4, 20, gorogoa)),
                ("The Bowl", listing(4, 20, gorogoa)),
            ],
            &[("The Boy", listing(41, 3, gorogoa))],
            gorogoa,
        ),
    ]
}

/// Everything priced by 17:31, each set looked up at the time `at` gives,
/// with the cards held and Heavy Rain's cards looked up in the order books.
fn price_book(at: impl Fn(u32) -> DateTime<Utc>) -> PriceBook {
    let mut book = PriceBook::default();
    for &(_, id, _, _, _, ev) in &QUEUE {
        book.sets.insert(id, queued_set(id, ev, at(id)));
    }
    book.sets.insert(
        COUNTER_STRIKE,
        queued_set(COUNTER_STRIKE, 3, at(COUNTER_STRIKE)),
    );
    for set in finished_sets(&at) {
        book.sets.insert(set.app_id, set);
    }
    for &(app_id, name, foil, ask, bid) in &BIDS {
        let when = self::at(17, 10);
        let hash = priced_card(app_id, name, foil, Price::Pending).market_hash_name;
        book.offers.insert(
            hash,
            Offers {
                price: order_book(ask, bid, when),
                looked_up_at: when,
            },
        );
    }
    let fruit = priced_card(GOROGOA, "The Fruit", false, Price::Pending).market_hash_name;
    book.offers.insert(
        fruit,
        Offers {
            price: Price::NoMarket,
            looked_up_at: self::at(17, 10),
        },
    );
    book
}

/// When each set was priced, in the ordinary run of things: Heavy Rain's at
/// 15:21, the rest through the afternoon, all under 6 hours old.
fn priced_at(app_id: u32) -> DateTime<Utc> {
    match app_id {
        HEAVY_RAIN => at(15, 21),
        HOLLOW_KNIGHT | INSCRYPTION => at(13, 2),
        _ => at(13, 21),
    }
}

// ── The states the mockups show ──────────────────────────────────────────────

/// Farming Heavy Rain alone at 17:31 (mockup a): its 17:23 card still being
/// found out, next look in 4 minutes.
pub(crate) fn farming_alone() -> DataSet {
    DataSet {
        status: FarmingStatus {
            status: Status::Farming,
            library: library(),
            order: order(),
            playing: vec![HEAVY_RAIN],
            mode: Some(Mode::Cards),
            blocked_by: None,
            next_look: Some(now() + TimeDelta::minutes(4)),
            look_every: Some(Duration::from_secs(5 * 60)),
            session: session(),
            set_aside: vec![SetAside {
                app_id: WARFRAME,
                times: 1,
                since: on(0, 4, 12) - TimeDelta::days(1),
            }],
            note: String::new(),
        },
        run: Run::Running,
        account: Some(Account {
            name: "alice".into(),
            expired: false,
        }),
        prefs: Preferences {
            skipped_games: vec![COUNTER_STRIKE],
            ..Default::default()
        },
        forecast: Some(forecast()),
        prices: price_book(priced_at),
        wallet: Some(pounds()),
        basis: Basis::List,
        pause: None,
        paused_since: None,
        now: now(),
        selected: HEAVY_RAIN,
    }
}

/// The queue scrolled to its end: Warframe, set aside, chosen.
pub(crate) fn queue_at_its_end() -> DataSet {
    DataSet {
        selected: WARFRAME,
        ..farming_alone()
    }
}

/// The 12 games short of 3 hours, played together since 17:24, when Stray
/// was ranked #1 (mockup b). LIMBO is #2.
pub(crate) fn building_hours() -> DataSet {
    let mut data = farming_alone();
    let group: Vec<u32> = std::iter::once(STRAY)
        .chain(
            QUEUE
                .iter()
                .filter(|g| g.2 > 0.0 && g.2 < 3.0 && g.1 != STRAY)
                .map(|g| g.1),
        )
        .collect();
    let rest = QUEUE
        .iter()
        .map(|g| g.1)
        .filter(|&id| id != STRAY && id != LIMBO);
    let order: Vec<u32> = [STRAY, LIMBO].into_iter().chain(rest).collect();
    // Hours built together reach Stray's 3 hours first; the rest is farmed
    // at the learnt rate, with what's left of building hours once more.
    let lead = 3.0 - 1.4;
    let games: Vec<(u32, f64, u32)> = order
        .iter()
        .filter_map(|&id| QUEUE.iter().find(|g| g.1 == id))
        .map(|&(_, id, hours, got, all, _)| {
            let built = if group.contains(&id) {
                hours + lead
            } else {
                hours
            };
            (id, built, all - got)
        })
        .collect();
    let plan = schedule(&games, |_| rate(), 3.0 - lead);
    let forecast = data.forecast.as_mut().expect("a forecast");
    forecast.per_game = plan
        .into_iter()
        .map(|(id, m)| (id, minutes(m + lead * 60.0)))
        .collect();
    data.prefs.priority_games = vec![STRAY, LIMBO];
    let status = &mut data.status;
    status.order = order;
    status.playing = group.clone();
    status.mode = Some(Mode::Hours);
    status.next_look = Some(now() + TimeDelta::minutes(48));
    status.look_every = None;
    let stretches = &mut status.session.stretches;
    if let Some(last) = stretches.last_mut() {
        last.to = Some(at(17, 24));
    }
    stretches.push(Stretch {
        app_ids: group,
        mode: Mode::Hours,
        from: at(17, 24),
        to: None,
    });
    data.selected = STRAY;
    data
}

/// Three minutes into the session (mockup c): no drops yet, 14 of the 62
/// games priced, and the time to finish a first estimate at 30 minutes a
/// drop.
pub(crate) fn first_minutes() -> DataSet {
    let start = at(9, 14);
    let mut games: Vec<Game> = STARTED_WITH
        .iter()
        .map(|&(name, id, hours, got, all, _)| game(name, id, hours, got, all))
        .collect();
    games.extend(
        QUEUE
            .iter()
            .filter(|g| g.1 != HEAVY_RAIN)
            .map(|&(name, id, hours, got, all, _)| game(name, id, hours, got, all)),
    );
    games[0].cards = cards(&[
        ("The Knight", 1),
        ("Hornet", 0),
        ("Zote", 0),
        ("Quirrel", 0),
        ("Cornifer", 0),
        ("Iselda", 0),
        ("Grimm", 0),
    ]);
    games.push(game("Counter-Strike 2", COUNTER_STRIKE, 350.0, 0, 2));
    let order: Vec<u32> = games.iter().take(62).map(|g| g.app_id).collect();
    let evs: Vec<i64> = STARTED_WITH
        .iter()
        .map(|g| g.5)
        .chain(QUEUE.iter().filter(|g| g.1 != HEAVY_RAIN).map(|g| g.5))
        .collect();
    let plan = schedule(
        &games
            .iter()
            .take(62)
            .map(|g| (g.app_id, g.hours, g.drops.remaining))
            .collect::<Vec<_>>(),
        |_| 2.0,
        3.0,
    );
    let mut prices = PriceBook::default();
    let finished = finished_sets(|_| start + TimeDelta::minutes(1));
    for (&id, &ev) in order.iter().zip(&evs).take(14) {
        let set = finished
            .iter()
            .find(|s| s.app_id == id)
            .cloned()
            .unwrap_or_else(|| queued_set(id, ev, start + TimeDelta::minutes(2)));
        prices.sets.insert(id, set);
    }
    // Heavy Rain's cards sold a little differently first thing: 5p a drop,
    // three drops to come.
    prices.sets.insert(
        HEAVY_RAIN,
        listed(
            HEAVY_RAIN,
            &[
                ("Ethan", 5),
                ("Carter", 5),
                ("Madison", 5),
                ("Norman", 6),
                ("Scott", 4),
            ],
            &[
                ("Ethan", 42),
                ("Carter", 35),
                ("Madison", 60),
                ("Norman", 51),
                ("Scott", 38),
            ],
            start + TimeDelta::minutes(2),
        ),
    );
    let now = at(9, 17);
    DataSet {
        status: FarmingStatus {
            status: Status::Farming,
            library: SteamLibrary::new(games),
            order,
            playing: vec![HOLLOW_KNIGHT],
            mode: Some(Mode::Cards),
            blocked_by: None,
            next_look: Some(now + TimeDelta::minutes(12)),
            look_every: Some(Duration::from_secs(15 * 60 + 15)),
            session: FarmingSession {
                started_at: start,
                drops_left_at_start: Some(252),
                games_at_start: Some(62),
                drops: Vec::new(),
                stretches: vec![Stretch {
                    app_ids: vec![HOLLOW_KNIGHT],
                    mode: Mode::Cards,
                    from: start,
                    to: None,
                }],
                finished: Vec::new(),
                first_forecast: None,
            },
            set_aside: vec![SetAside {
                app_id: WARFRAME,
                times: 1,
                since: on(0, 4, 12) - TimeDelta::days(1),
            }],
            note: String::new(),
        },
        forecast: Some(Forecast {
            eta: hours(5 * 24 + 9),
            band: None,
            assumed: true,
            hours_term: hours(3),
            rate: 2.0,
            per_game: plan.into_iter().map(|(id, m)| (id, minutes(m))).collect(),
            made_at: now,
        }),
        prices,
        now,
        selected: HOLLOW_KNIGHT,
        ..farming_alone()
    }
}

/// Signed on at 09:14 and reading the badges for the first time: nothing is
/// known yet (mockups c, at three sizes).
pub(crate) fn reading_badges() -> DataSet {
    let start = at(9, 14);
    DataSet {
        status: FarmingStatus {
            status: Status::Checking,
            note: "reading your badges…".into(),
            session: FarmingSession {
                started_at: start,
                ..Default::default()
            },
            ..Default::default()
        },
        forecast: None,
        prices: PriceBook::default(),
        now: start + TimeDelta::seconds(20),
        ..farming_alone()
    }
}

/// Hades played on the user's PC since 17:29 (mockup d): farming waits.
pub(crate) fn waiting_for_hades() -> DataSet {
    let mut data = farming_alone();
    let status = &mut data.status;
    status.status = Status::Blocked;
    status.playing = Vec::new();
    status.mode = None;
    status.blocked_by = Some(HADES);
    status.next_look = None;
    status.look_every = None;
    status.note = "playing on another device — farming waits until it stops".into();
    stop_at(&mut status.session, at(17, 29));
    data
}

/// Paused by the user at 17:31 (mockup f).
pub(crate) fn paused() -> DataSet {
    let mut data = farming_alone();
    data.run = Run::Paused;
    stop_at(&mut data.status.session, now());
    data
}

/// Steam stopped taking the saved sign-in at 17:31 (mockup g): farming
/// stopped, the queue and this session's cards kept.
pub(crate) fn sign_in_expired() -> DataSet {
    let mut data = farming_alone();
    if let Some(account) = &mut data.account {
        account.expired = true;
    }
    let status = &mut data.status;
    status.status = Status::Error;
    status.playing = Vec::new();
    status.mode = None;
    status.next_look = None;
    status.look_every = None;
    status.note = "Steam rejected the saved sign-in".into();
    stop_at(&mut status.session, now());
    data
}

/// The connection to Steam went at 17:30; it's tried again in 42 seconds
/// (mockup g).
pub(crate) fn reconnecting() -> DataSet {
    let mut data = farming_alone();
    let status = &mut data.status;
    status.status = Status::Error;
    status.playing = Vec::new();
    status.mode = None;
    status.next_look = Some(now() + TimeDelta::seconds(42));
    status.look_every = None;
    status.note = "the connection was reset".into();
    stop_at(&mut status.session, at(17, 30));
    data
}

/// Steam has paused price lookups since 13:31, doubling to an hour: the
/// next try is at 17:41 (mockup h). The sets looked up before 11:31 are
/// over 6 hours old; Hades' and Inscryption's were looked up since.
pub(crate) fn prices_paused() -> DataSet {
    let stale = |app_id: u32| match app_id {
        HOLLOW_KNIGHT => now() - TimeDelta::hours(7),
        HADES | INSCRYPTION => at(13, 2),
        _ => now() - TimeDelta::hours(8),
    };
    DataSet {
        prices: price_book(stale),
        pause: Some(MarketPause {
            until: at(17, 41),
            step: Duration::from_secs(60 * 60),
        }),
        paused_since: Some(at(13, 31)),
        ..farming_alone()
    }
}

/// The last stretch ended then.
fn stop_at(session: &mut FarmingSession, when: DateTime<Utc>) {
    if let Some(last) = session.stretches.last_mut() {
        last.to = Some(when);
    }
}

// ── Five days on ─────────────────────────────────────────────────────────────

/// When the last games' last cards dropped, the newest last: Sunday's and
/// Saturday evening's, as the Done section shows them.
const LAST_FINISHED: [(&str, Day); 11] = [
    ("Northgard", (4, 16, 58)),
    ("Oddworld: Soulstorm", (4, 19, 24)),
    ("Pentiment", (4, 21, 50)),
    ("Raft", (5, 0, 13)),
    ("Return of the Obra Dinn", (5, 2, 47)),
    ("Slay the Spire", (5, 5, 2)),
    ("Spiritfarer", (5, 7, 40)),
    ("Terraria", (5, 9, 58)),
    ("Valheim", (5, 12, 31)),
    ("Warframe", (5, 15, 12)),
    ("Vampire Survivors", (5, 17, 44)),
];

/// Hours on record once done, for the games the Done section shows.
const DONE_HOURS: [(&str, f64); 11] = [
    ("Vampire Survivors", 5.1),
    ("Valheim", 3.9),
    ("Terraria", 4.1),
    ("Spiritfarer", 3.8),
    ("Slay the Spire", 3.6),
    ("Return of the Obra Dinn", 3.9),
    ("Raft", 4.2),
    ("Pentiment", 3.7),
    ("Oddworld: Soulstorm", 4.0),
    ("Northgard", 3.6),
    ("Warframe", 18.4),
];

/// The last cards: Warframe's and Vampire Survivors', and which copy each
/// was. The last Antonio is a second copy.
const LAST_DROPS: [(Day, u32, &str, u32); 9] = [
    ((5, 13, 24), WARFRAME, "Loki", 1),
    ((5, 14, 0), WARFRAME, "Volt", 1),
    ((5, 14, 36), WARFRAME, "Excalibur", 1),
    ((5, 15, 12), WARFRAME, "Mag", 1),
    ((5, 15, 49), VAMPIRE_SURVIVORS, "Antonio", 1),
    ((5, 16, 20), VAMPIRE_SURVIVORS, "Imelda", 1),
    ((5, 16, 51), VAMPIRE_SURVIVORS, "Pasqualina", 1),
    ((5, 17, 18), VAMPIRE_SURVIVORS, "Gennaro", 1),
    ((5, 17, 44), VAMPIRE_SURVIVORS, "Antonio", 2),
];

/// Five days on, at 17:44 on Sunday, every card has dropped (mockup e):
/// 252 drops from 62 games, two of them foils, Thanatos and Cult of the
/// Lamb's The Lamb; ≥ £17.31 with 4 not priced. Counter-Strike 2, skipped,
/// still has 2 to drop. Farming looks again at 01:44.
pub(crate) fn nothing_to_farm() -> DataSet {
    let base = farming_alone();
    let now = on(5, 17, 44);
    // The farm order as it stood at the start, Warframe set aside behind
    // the rest and Vampire Survivors, set aside later, last of all.
    let mut order: Vec<u32> = STARTED_WITH.iter().map(|g| g.1).collect();
    order.extend(
        QUEUE
            .iter()
            .map(|g| g.1)
            .filter(|&id| id != HEAVY_RAIN && id != WARFRAME && id != VAMPIRE_SURVIVORS),
    );
    order.extend([WARFRAME, VAMPIRE_SURVIVORS]);
    let name_of = |id: u32| {
        QUEUE
            .iter()
            .map(|g| (g.0, g.1))
            .chain(STARTED_WITH.iter().map(|g| (g.0, g.1)))
            .find(|g| g.1 == id)
            .map_or("", |g| g.0)
    };

    // When each game finished: this session's first five as they did on
    // Tuesday, Heavy Rain at 17:55, then the rest through the week, Cult of
    // the Lamb on Friday morning, and the last eleven as the Done section
    // shows them.
    let mut finished_at: Vec<(u32, DateTime<Utc>)> = DONE
        .iter()
        .map(|&(_, id, _, _, _, (h, m))| (id, at(h, m)))
        .collect();
    finished_at.push((HEAVY_RAIN, at(17, 55)));
    let spread =
        |from: DateTime<Utc>, to: DateTime<Utc>, ids: &[u32]| -> Vec<(u32, DateTime<Utc>)> {
            let step = (to - from) / i32::try_from(ids.len()).unwrap_or(1);
            ids.iter()
                .enumerate()
                .map(|(i, &id)| (id, from + step * (i32::try_from(i).unwrap_or(0) + 1)))
                .collect()
        };
    let middle: Vec<u32> = order[6..]
        .iter()
        .copied()
        .filter(|&id| !LAST_FINISHED.iter().any(|(n, _)| *n == name_of(id)))
        .collect();
    let cult = middle
        .iter()
        .position(|&id| id == CULT_OF_THE_LAMB)
        .unwrap_or(0);
    finished_at.extend(spread(at(17, 55), on(3, 10, 0), &middle[..cult]));
    finished_at.push((CULT_OF_THE_LAMB, on(3, 11, 20)));
    finished_at.extend(spread(on(3, 11, 20), on(4, 14, 40), &middle[cult + 1..]));
    for (name, (d, h, m)) in LAST_FINISHED {
        let id = order
            .iter()
            .copied()
            .find(|&id| name_of(id) == name)
            .unwrap_or(0);
        finished_at.push((id, on(d, h, m)));
    }

    // Every drop: this session's first 16, as they were but for the 17:23
    // card, which nothing could tell, and Madeline, whose lookup failed;
    // then each game's drops left, half an hour apart, up to its last.
    let mut drops = drops();
    drops[15].card = DropCard::Unknown;
    drops.push(Drop {
        at: at(17, 55),
        app_id: HEAVY_RAIN,
        card: DropCard::Identified(asset(32_000, HEAVY_RAIN, "Ethan", false)),
        copy: Some(1),
    });
    let mut next_asset = 32_001;
    for &(id, done) in &finished_at {
        let Some(&(_, _, _, got, all, _)) = QUEUE.iter().find(|g| g.1 == id) else {
            continue;
        };
        if id == HEAVY_RAIN || id == WARFRAME || id == VAMPIRE_SURVIVORS {
            continue;
        }
        let left = all - got;
        for i in 0..left {
            let at = done - TimeDelta::minutes(31 * i64::from(left - 1 - i));
            let foil = id == CULT_OF_THE_LAMB && i == left - 1;
            let name = if foil {
                "The Lamb".to_owned()
            } else {
                format!("Card {}", i % 5 + 1)
            };
            drops.push(Drop {
                at,
                app_id: id,
                card: DropCard::Identified(asset(next_asset, id, &name, foil)),
                copy: Some(1),
            });
            next_asset += 1;
        }
    }
    for &((d, h, m), id, name, copy) in &LAST_DROPS {
        drops.push(Drop {
            at: on(d, h, m),
            app_id: id,
            card: DropCard::Identified(asset(next_asset, id, name, false)),
            copy: Some(copy),
        });
        next_asset += 1;
    }
    drops.sort_by_key(|d| d.at);

    // Every game farmed is done; Counter-Strike 2, skipped, isn't.
    let mut games: Vec<Game> = order
        .iter()
        .map(|&id| {
            let (name, hours, total) = QUEUE
                .iter()
                .map(|g| (g.0, g.1, g.2, g.4))
                .chain(DONE.iter().map(|g| (g.0, g.1, g.2, g.3)))
                .find(|g| g.1 == id)
                .map(|g| (g.0, g.2, g.3))
                .unwrap_or(("", 0.0, 0));
            let hours = DONE_HOURS
                .iter()
                .find(|(n, _)| *n == name)
                .map_or(hours.max(4.0), |h| h.1);
            game(name, id, hours, total, total)
        })
        .collect();
    if let Some(vs) = games.iter_mut().find(|g| g.app_id == VAMPIRE_SURVIVORS) {
        vs.cards = cards(&[
            ("Antonio", 2),
            ("Imelda", 1),
            ("Pasqualina", 1),
            ("Gennaro", 1),
            ("Arca", 0),
            ("Porta", 0),
        ]);
    }
    games.push(game("Counter-Strike 2", COUNTER_STRIKE, 350.0, 0, 2));

    // Prices: each card of the games past Heavy Rain at its game's drop
    // value, and The Lamb at 65p. Madeline's lookup failed, and so did one
    // of Loop Hero's cards'.
    let when = now - TimeDelta::hours(2);
    let mut prices = price_book(|_| when);
    for &(_, id, _, _, _, ev) in &QUEUE {
        if id == HEAVY_RAIN {
            continue;
        }
        let names: Vec<(String, i64)> = (1..=5).map(|i| (format!("Card {i}"), ev)).collect();
        let normal: Vec<(&str, i64)> = names.iter().map(|(n, p)| (n.as_str(), *p)).collect();
        let foil: &[(&str, i64)] = if id == CULT_OF_THE_LAMB {
            &[("The Lamb", 65)]
        } else {
            &[]
        };
        prices.sets.insert(id, listed(id, &normal, foil, when));
    }
    prices.sets.insert(
        WARFRAME,
        listed(
            WARFRAME,
            &[
                ("Loki", 4),
                ("Volt", 5),
                ("Excalibur", 4),
                ("Mag", 3),
                ("Rhino", 4),
            ],
            &[],
            when,
        ),
    );
    prices.sets.insert(
        VAMPIRE_SURVIVORS,
        listed(
            VAMPIRE_SURVIVORS,
            &[
                ("Antonio", 5),
                ("Imelda", 4),
                ("Pasqualina", 5),
                ("Gennaro", 4),
                ("Arca", 6),
                ("Porta", 5),
            ],
            &[],
            when,
        ),
    );
    if let Some(celeste) = prices.sets.get_mut(&CELESTE) {
        celeste.normal[0].price = Price::failed(when);
    }
    let loop_hero = QUEUE.iter().find(|g| g.0 == "Loop Hero").map_or(0, |g| g.1);
    if let Some(set) = prices.sets.get_mut(&loop_hero) {
        set.normal[2].price = Price::failed(when);
    }

    let stretches: Vec<Stretch> = {
        let mut from = at(9, 14);
        let mut sorted = finished_at.clone();
        sorted.sort_by_key(|f| f.1);
        sorted
            .iter()
            .map(|&(id, to)| {
                let s = Stretch {
                    app_ids: vec![id],
                    mode: Mode::Cards,
                    from,
                    to: Some(to),
                };
                from = to;
                s
            })
            .collect()
    };
    let mut finished: Vec<Finished> = finished_at
        .iter()
        .map(|&(app_id, at)| Finished { app_id, at })
        .collect();
    finished.sort_by_key(|f| f.at);

    DataSet {
        status: FarmingStatus {
            status: Status::Idle,
            library: SteamLibrary::new(games),
            order: Vec::new(),
            playing: Vec::new(),
            mode: None,
            blocked_by: None,
            next_look: Some(now + TimeDelta::hours(8)),
            look_every: None,
            session: FarmingSession {
                drops,
                stretches,
                finished,
                ..session()
            },
            set_aside: Vec::new(),
            note: "every game with cards left is skipped".into(),
        },
        forecast: Some(Forecast {
            eta: Duration::ZERO,
            band: None,
            assumed: false,
            hours_term: Duration::ZERO,
            rate: 2.1,
            per_game: Vec::new(),
            made_at: now,
        }),
        prices,
        now,
        selected: VAMPIRE_SURVIVORS,
        ..base
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::format;

    #[test]
    fn the_library_is_the_spec_s() {
        let library = library();
        assert_eq!(library.games().len(), 63);
        assert_eq!(
            (library.drops_received(), library.drops_total()),
            (183, 421)
        );
        assert_eq!(library.games_done(), 5);
        let queue: u32 = QUEUE.iter().map(|g| g.4 - g.3).sum();
        assert_eq!(queue, 236);
    }

    #[test]
    fn the_session_learnt_from_16_drops_in_7h_40m_farming_alone() {
        let session = session();
        assert_eq!(session.drops.len(), 16);
        let alone: i64 = session
            .stretches
            .iter()
            .map(|s| (s.to.unwrap_or(now()) - s.from).num_minutes())
            .sum();
        assert_eq!(alone, 7 * 60 + 40);
        assert!(
            session
                .drops
                .iter()
                .all(|d| session
                    .stretches
                    .iter()
                    .any(|s| s.app_ids.contains(&d.app_id)
                        && s.from <= d.at
                        && d.at <= s.to.unwrap_or(now()))),
            "every drop came farming alone"
        );
        assert_eq!(format::rate(rate()), "2.1 drops an hour");
    }

    #[test]
    fn the_forecast_is_the_spec_s() {
        let f = forecast();
        assert_eq!(format::eta(f.eta), "≈ 4d 21h");
        assert_eq!(format::band(f.band.unwrap()), "80%: 3d 13h – 6d 15h");
        let at = |i: usize| format::estimate(f.per_game[i].1);
        assert_eq!(
            [at(0), at(1), at(2), at(8), at(14), at(56)],
            ["25m", "1h 30m", "2h 30m", "11h", "1d 1h", "4d 21h"]
        );
    }

    #[test]
    fn each_queued_sets_cards_average_a_drops_value() {
        let wallet = pounds();
        let book = price_book(priced_at);
        for &(name, id, _, _, _, ev) in &QUEUE {
            let each = market::expected_per_drop(&book.sets[&id], Basis::List, &wallet);
            assert_eq!(each.map(|m| m.minor), Some(ev), "{name}");
        }
        assert_eq!(around(6).0, [4, 9, 6, 6, 5], "LIMBO's cards: £0.04–0.09");
        assert_eq!(around(6).1[..2], [49, 135], "and its foils: £0.49–1.35");
    }

    #[test]
    fn five_days_on_every_card_has_dropped() {
        let data = nothing_to_farm();
        let s = data.snapshot();
        let session = &data.status.session;
        assert_eq!(session.drops.len(), 252);
        assert_eq!(session.finished.len(), 62);
        assert_eq!(
            session.drops.iter().filter(|d| d.card.is_foil()).count(),
            2,
            "Thanatos and The Lamb"
        );
        let library = &data.status.library;
        assert_eq!(
            (library.drops_received(), library.drops_total()),
            (419, 421)
        );
        let all: Vec<&Drop> = session.drops.iter().collect();
        let held = crate::viewmodel::session_value(&s, &all, Basis::List).unwrap();
        assert_eq!(format::held(&held), "≥ £17.31 · 4 unpriced");
        assert_eq!(
            session.finished.last().map(|f| f.app_id),
            Some(VAMPIRE_SURVIVORS)
        );
        assert!(session.drops.windows(2).all(|w| w[0].at <= w[1].at));
    }
}
