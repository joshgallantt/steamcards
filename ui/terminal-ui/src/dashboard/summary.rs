use std::time::Duration;

use account::Wallet;
use card::{
    Basis, CardKind, HeldCard, Price, PriceBook, held_value, on_completion, value_left, value_of,
};
use chrono::{DateTime, Utc};
use game::{AppId, Game, SteamLibrary};
use money::Money;
use session::{DropCard, Forecast, Session};

/// Cards are valued at their market price: what buyers pay, the lowest
/// listing on the Steam market.
const BASIS: Basis = Basis::List;

/// An amount of money, and whether some cards weren't priced yet, so that
/// it's at least that much.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Value {
    pub money: Money,
    pub partial: bool,
}

/// The dashboard's summary: this session, what's left, and every game.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// How long this session has played: waiting for another device, and
    /// pauses, left out.
    pub session_played: Duration,
    /// The cards that dropped this session, each copy counted.
    pub session_cards: usize,
    /// What they're worth, once any is priced.
    pub session_value: Option<Value>,
    /// Card drops still to come in the games that will be farmed.
    pub cards_to_go: u32,
    pub games_to_go: usize,
    /// About how long farming them takes: learnt from this session's drops,
    /// assuming half an hour a drop until it has two to go by.
    pub time_to_go: Option<Duration>,
    /// Every game with cards: the drops received, and all there are.
    pub all_received: u32,
    pub all_total: u32,
    /// About what this session's cards and those still to come will be worth
    /// once every card has dropped.
    pub when_done: Option<Value>,
}

impl Summary {
    /// `order` is the games that will be farmed, in farm order, on an
    /// account that holds cards back for `before_drops` hours.
    pub fn build(
        session: &Session,
        library: &SteamLibrary,
        order: &[AppId],
        before_drops: u8,
        book: &PriceBook,
        wallet: Option<&Wallet>,
        now: DateTime<Utc>,
    ) -> Self {
        let farmed: Vec<&Game> = order
            .iter()
            .filter_map(|&id| library.game(id))
            .filter(|g| g.has_drops_left())
            .collect();
        let cards_to_go = farmed.iter().map(|g| g.drops.remaining).sum();
        let time_to_go =
            (cards_to_go > 0).then(|| Forecast::of(session, library, order, before_drops, now).eta);

        let (session_value, when_done) = match wallet {
            Some(wallet) => {
                let (cards, unidentified) = held_cards(session);
                let held = held_value(&cards, unidentified, book, BASIS, wallet, now);
                let left = value_left(library, order, book, BASIS, wallet);
                let done = on_completion(&held, &left);
                let session_value = (held.priced > 0).then_some(Value {
                    money: held.total,
                    partial: held.unpriced > 0,
                });
                let priced_any = held.priced > 0 || left.unpriced_games < farmed.len() as u32;
                let when_done = priced_any.then_some(Value {
                    money: done.value,
                    partial: held.unpriced > 0 || left.unpriced_games > 0,
                });
                (session_value, when_done)
            }
            None => (None, None),
        };

        Self {
            session_played: session.time_played(now),
            session_cards: session.drops.len(),
            session_value,
            cards_to_go,
            games_to_go: farmed.len(),
            time_to_go,
            all_received: library.drops_received(),
            all_total: library.drops_total(),
            when_done,
        }
    }
}

/// This session's cards as the market values them, and how many drops
/// aren't known yet: still being found out, or nothing could tell.
fn held_cards(session: &Session) -> (Vec<HeldCard>, u32) {
    let mut cards = Vec::new();
    let mut unknown = 0;
    for drop in &session.drops {
        match &drop.card {
            DropCard::Identified(asset) => cards.push(HeldCard::from(asset)),
            DropCard::NameOnly { name } => {
                cards.push(HeldCard::named(drop.app_id, name, CardKind::Normal));
            }
            DropCard::Identifying | DropCard::Unknown => unknown += 1,
        }
    }
    (cards, unknown)
}

/// What a card is worth, as a list shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardPrice {
    Worth(Money),
    /// Not priced yet: it's on its way.
    Waiting,
    /// No price to be had: nobody's selling it, it can't be sold, or the
    /// lookup failed.
    None,
}

impl CardPrice {
    fn of(price: &Price, wallet: &Wallet) -> Self {
        match (value_of(price, BASIS, wallet), price) {
            (Some(money), _) => Self::Worth(money),
            (None, Price::Pending) => Self::Waiting,
            (None, _) => Self::None,
        }
    }
}

/// Which card dropped, as far as it's known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardName {
    /// `spare` is which copy of it the account then held, when the session
    /// says it's a spare: a card already had.
    Named {
        name: String,
        kind: CardKind,
        spare: Option<u32>,
    },
    /// Still being found out: a moment after it dropped.
    Finding,
    /// Nothing could tell which card it was.
    Unknown,
}

/// A card that dropped this session, as its list shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCard {
    pub at: DateTime<Utc>,
    pub game: String,
    pub card: CardName,
    pub price: CardPrice,
}

/// This session's cards, newest first.
pub fn session_cards(
    session: &Session,
    library: &SteamLibrary,
    book: &PriceBook,
    wallet: Option<&Wallet>,
) -> Vec<SessionCard> {
    session
        .drops
        .iter()
        .rev()
        .map(|drop| {
            let game = library
                .game(drop.app_id)
                .map_or_else(|| format!("App {}", drop.app_id), |g| g.name.clone());
            let held = match &drop.card {
                DropCard::Identified(asset) => Some(HeldCard::from(asset)),
                DropCard::NameOnly { name } => {
                    Some(HeldCard::named(drop.app_id, name, CardKind::Normal))
                }
                DropCard::Identifying | DropCard::Unknown => None,
            };
            let card = match (&drop.card, &held) {
                (DropCard::Identifying, _) => CardName::Finding,
                (_, Some(h)) => CardName::Named {
                    name: h.name.clone(),
                    kind: h.kind,
                    spare: drop.copy.filter(|_| drop.is_spare()),
                },
                (_, None) => CardName::Unknown,
            };
            let price = match (held, wallet) {
                (Some(h), Some(w)) => CardPrice::of(&book.price(&h), w),
                (None, _) if drop.card == DropCard::Unknown => CardPrice::None,
                _ => CardPrice::Waiting,
            };
            SessionCard {
                at: drop.at,
                game,
                card,
                price,
            }
        })
        .collect()
}

/// What a game's cards still to drop are likely worth: its drops left, each
/// at the average price of its set's cards. `None` when it has none left.
pub fn value_to_come(game: &Game, book: &PriceBook, wallet: Option<&Wallet>) -> Option<CardPrice> {
    if !game.has_drops_left() {
        return None;
    }
    let Some(wallet) = wallet else {
        return Some(CardPrice::Waiting);
    };
    let Some(set) = book.sets.get(&game.app_id) else {
        return Some(CardPrice::Waiting);
    };
    let one = SteamLibrary::new(vec![game.clone()]);
    let left = value_left(&one, &[game.app_id], book, BASIS, wallet);
    Some(if left.unpriced_games > 0 {
        let failed = set.retry_at.is_some();
        if failed || set.normal.iter().all(|c| c.price != Price::Pending) {
            CardPrice::None
        } else {
            CardPrice::Waiting
        }
    } else {
        CardPrice::Worth(left.value)
    })
}

/// A card of a game's set, by name: what it's worth.
pub fn card_price(
    app_id: AppId,
    name: &str,
    book: &PriceBook,
    wallet: Option<&Wallet>,
) -> CardPrice {
    let (Some(set), Some(wallet)) = (book.sets.get(&app_id), wallet) else {
        return CardPrice::Waiting;
    };
    CardPrice::of(&set.price(name, CardKind::Normal), wallet)
}

/// The games to price, most urgent first: those playing now, then those
/// whose cards dropped this session (newest first), then the rest in farm
/// order.
pub fn prices_wanted(playing: &[AppId], session: &Session, order: &[AppId]) -> Vec<AppId> {
    let mut wanted: Vec<AppId> = Vec::new();
    let dropped = session.drops.iter().rev().map(|d| d.app_id);
    for id in playing
        .iter()
        .copied()
        .chain(dropped)
        .chain(order.iter().copied())
    {
        if !wanted.contains(&id) {
            wanted.push(id);
        }
    }
    wanted
}

#[cfg(test)]
mod tests {
    use card::{
        AssetId, CardAsset, PriceQuote, PricedCard, SetPrices,
        test_support::{listing, pounds},
    };
    use chrono::TimeZone;
    use game::CardDrops;
    use money::Currency;
    use session::{Drop, Mode, Stretch};

    use super::*;

    fn at(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 29, h, m, 0).unwrap()
    }

    fn heavy_rain() -> Game {
        Game {
            app_id: AppId(960_910),
            name: "Heavy Rain".into(),
            hours: 4.0,
            drops: CardDrops {
                received: 3,
                remaining: 1,
            },
            badge_level: 0,
            private: false,
            bought_at: None,
        }
    }

    fn limbo() -> Game {
        Game {
            app_id: AppId(48_000),
            name: "LIMBO".into(),
            hours: 3.4,
            drops: CardDrops {
                received: 3,
                remaining: 2,
            },
            badge_level: 0,
            private: false,
            bought_at: None,
        }
    }

    fn priced(app_id: u32, cards: &[(&str, i64)]) -> SetPrices {
        SetPrices {
            app_id: AppId(app_id),
            normal: cards
                .iter()
                .map(|&(name, pence)| PricedCard {
                    name: name.into(),
                    market_hash_name: format!("{app_id}-{name}"),
                    price: listing(pence, 10, at(17, 0)),
                })
                .collect(),
            foil: Vec::new(),
            fetched_at: at(17, 0),
            retry_at: None,
        }
    }

    fn book() -> PriceBook {
        let mut book = PriceBook::default();
        for set in [
            priced(
                960_910,
                &[
                    ("Ethan", 5),
                    ("Carter", 4),
                    ("Madison", 5),
                    ("Norman", 6),
                    ("Scott", 4),
                ],
            ),
            priced(48_000, &[("Boy", 6), ("Sister", 6)]),
        ] {
            book.sets.insert(set.app_id, set);
        }
        book
    }

    fn madison(asset_id: u64) -> CardAsset {
        CardAsset {
            asset_id: AssetId(asset_id),
            app_id: AppId(960_910),
            name: "Madison".into(),
            market_hash_name: "960910-Madison".into(),
            kind: CardKind::Normal,
            marketable: true,
            tradable: true,
        }
    }

    /// Two Madisons, the second a spare, and a card still being found out.
    fn session() -> Session {
        Session {
            started_at: at(16, 50),
            drops: vec![
                Drop {
                    at: at(17, 5),
                    app_id: AppId(960_910),
                    card: DropCard::Identified(madison(1)),
                    copy: Some(1),
                },
                Drop {
                    at: at(17, 10),
                    app_id: AppId(960_910),
                    card: DropCard::Identified(madison(2)),
                    copy: Some(2),
                },
                Drop {
                    at: at(17, 23),
                    app_id: AppId(960_910),
                    card: DropCard::Identifying,
                    copy: None,
                },
            ],
            stretches: vec![Stretch {
                app_ids: vec![AppId(960_910)],
                mode: Mode::Cards,
                from: at(16, 50),
                to: None,
            }],
            ..Default::default()
        }
    }

    fn money(pence: i64) -> Money {
        Money::new(pence, Currency::from_id(2))
    }

    #[test]
    fn the_summary_counts_every_copy_and_says_when_some_arent_priced() {
        let library = SteamLibrary::new(vec![heavy_rain(), limbo()]);
        let s = Summary::build(
            &session(),
            &library,
            &[AppId(960_910), AppId(48_000)],
            3,
            &book(),
            Some(&pounds()),
            at(17, 31),
        );
        assert_eq!(s.session_cards, 3);
        assert_eq!(
            s.session_value,
            Some(Value {
                money: money(10),
                partial: true
            })
        );
        assert_eq!((s.cards_to_go, s.games_to_go), (3, 2));
        assert_eq!((s.all_received, s.all_total), (6, 9));
        // Heavy Rain: one drop at the set's average, 5p; LIMBO: two at 6p.
        assert_eq!(
            s.when_done,
            Some(Value {
                money: money(10 + 5 + 12),
                partial: true
            })
        );
        assert!(s.time_to_go.is_some());
    }

    #[test]
    fn no_money_is_shown_before_the_wallet_is_known() {
        let library = SteamLibrary::new(vec![heavy_rain()]);
        let s = Summary::build(
            &session(),
            &library,
            &[AppId(960_910)],
            3,
            &book(),
            None,
            at(17, 31),
        );
        assert_eq!((s.session_value, s.when_done), (None, None));
    }

    #[test]
    fn nothing_to_go_means_no_time_to_go() {
        let mut done = heavy_rain();
        done.drops = CardDrops {
            received: 4,
            remaining: 0,
        };
        let library = SteamLibrary::new(vec![done]);
        let s = Summary::build(
            &Session::default(),
            &library,
            &[],
            3,
            &book(),
            Some(&pounds()),
            at(17, 31),
        );
        assert_eq!((s.cards_to_go, s.games_to_go, s.time_to_go), (0, 0, None));
        assert_eq!(s.session_value, None);
    }

    #[test]
    fn the_sessions_cards_come_newest_first_with_spares_marked() {
        let library = SteamLibrary::new(vec![heavy_rain()]);
        let cards = session_cards(&session(), &library, &book(), Some(&pounds()));
        assert_eq!(cards.len(), 3);
        assert_eq!(cards[0].card, CardName::Finding);
        assert_eq!(cards[0].price, CardPrice::Waiting);
        assert_eq!(
            cards[1].card,
            CardName::Named {
                name: "Madison".into(),
                kind: CardKind::Normal,
                spare: Some(2)
            }
        );
        assert_eq!(cards[1].price, CardPrice::Worth(money(5)));
        assert_eq!(cards[2].game, "Heavy Rain");
    }

    #[test]
    fn a_games_cards_to_come_are_valued_at_its_sets_average() {
        let w = pounds();
        assert_eq!(
            value_to_come(&limbo(), &book(), Some(&w)),
            Some(CardPrice::Worth(money(12)))
        );
        let mut unpriced = limbo();
        unpriced.app_id = AppId(1);
        assert_eq!(
            value_to_come(&unpriced, &book(), Some(&w)),
            Some(CardPrice::Waiting)
        );
        let mut done = limbo();
        done.drops.remaining = 0;
        assert_eq!(value_to_come(&done, &book(), Some(&w)), None);
    }

    #[test]
    fn a_set_nobody_sells_has_no_value_to_come() {
        let mut book = PriceBook::default();
        let mut set = priced(48_000, &[("Boy", 6)]);
        set.normal[0].price = Price::NoMarket;
        book.sets.insert(AppId(48_000), set);
        assert_eq!(
            value_to_come(&limbo(), &book, Some(&pounds())),
            Some(CardPrice::None)
        );
    }

    #[test]
    fn a_card_of_the_set_is_valued_by_its_name() {
        let w = pounds();
        assert_eq!(
            card_price(AppId(960_910), "Norman", &book(), Some(&w)),
            CardPrice::Worth(money(6))
        );
        assert_eq!(
            card_price(AppId(1), "Norman", &book(), Some(&w)),
            CardPrice::Waiting
        );
        let mut book = book();
        book.sets.get_mut(&AppId(960_910)).unwrap().normal[0].price = Price::Known(PriceQuote {
            ask: None,
            ask_depth: None,
            fetched_at: at(17, 0),
        });
        assert_eq!(
            card_price(AppId(960_910), "Ethan", &book, Some(&w)),
            CardPrice::None
        );
    }

    #[test]
    fn prices_are_wanted_for_whats_playing_then_whats_dropped_then_the_rest() {
        let wanted = prices_wanted(&[AppId(48_000)], &session(), &[1, 48_000, 2].map(AppId));
        assert_eq!(wanted, [48_000, 960_910, 1, 2].map(AppId));
    }
}
