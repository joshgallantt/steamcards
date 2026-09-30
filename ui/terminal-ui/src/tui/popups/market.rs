// The market (docs/design/ui.md, mockup h): where every price comes from.
// Each game's card and foil range, its value a drop and still to drop, and
// when it was priced, in farm order and then the games with cards this
// session; what each price state means; the cards this session that aren't
// priced and why; and, while Steam has paused lookups, a banner that says
// so. Later it gains the tabs Listings and Quick-sell.

use std::time::Duration;

use market::{Basis, Currency, Money};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx, format,
        text::{Fits, Hint, first_fit, fit, join, rfit, rule, rule_ladder, spread, width, wrap},
        theme,
    },
    Scroll, Shown, chosen, game_name,
};
use crate::viewmodel::{Banner, Cell, MarketRow, MarketView, Priced, Unpriced, Why};

/// The chosen game, by app ID, so it stays put as prices come in (the
/// dashboard's chosen game, or the first, when `None`); the first line of
/// the games shown; and how far the pop-up is scrolled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in super::super) struct MarketList {
    pub(super) game: Option<u32>,
    top: usize,
    pub(super) scroll: Scroll,
}

/// From this wide inside, the games' table has every column and the
/// legend stands beside it.
const WIDE: usize = LIST + 3 + SIDE;
/// The table's columns, when every one shows.
const COLUMNS: [(&str, usize); 6] = [
    ("GAME", 24),
    ("CARDS", 12),
    ("FOILS", 12),
    ("≈ A DROP", 10),
    ("≈ LEFT", 8),
    ("PRICED", 10),
];
const LIST: usize = 24 + 12 + 12 + 10 + 8 + 10;
/// The legend beside the table.
const SIDE: usize = 35;
/// A price in the legend, and what it means beside it.
const MARK: usize = 13;
/// Games shown at a time, when the legend goes below the table.
const NARROW_ROWS: usize = 10;

const KEYS: [Hint; 5] = [
    Hint::new("↑↓", "choose", 1),
    Hint::new("b", "basis", 3),
    Hint::new("o", "on the market", 4),
    Hint::new("r", "look again", 5),
    Hint::new("esc", "close", 0),
];

/// The games listed, in order, by app ID: farm order, then this session's.
pub(super) fn games(view: &MarketView) -> Vec<u32> {
    view.rows
        .iter()
        .chain(&view.session_rows)
        .map(|r| r.app_id)
        .collect()
}

/// Where the cursor is among `games`: on `game`, else the dashboard's
/// chosen game, else the first.
pub(super) fn cursor(games: &[u32], game: Option<u32>, selected: Option<u32>) -> usize {
    game.or(selected)
        .and_then(|id| games.iter().position(|&g| g == id))
        .unwrap_or(0)
}

pub(super) fn shown(cx: &Ctx<'_>, area: Rect, v: &mut MarketList) -> Fits<Shown> {
    let w = usize::from(area.width);
    let inner = w.saturating_sub(6);
    let view = MarketView::build(&cx.s, cx.app.market.paused_since());
    let at = cursor(&games(&view), v.game, cx.app.selected);
    let currency = cx.s.wallet.map_or(Currency::USD, |w| w.currency);
    let room = usize::from(area.height).saturating_sub(2);
    let (lines, focus) = lines(&view, at, &mut v.top, currency, inner, room)?;
    let mut shown = Shown::new("Market · Prices", lines, &KEYS);
    shown.focus = focus;
    let what = if view.rows.is_empty() {
        "nothing left to farm".to_owned()
    } else {
        format!(
            "{} with drops left, in farm order",
            format::games(view.rows.len())
        )
    };
    shown.note = Some(Line::styled(
        format!("{} · {what}", basis_words(view.basis)),
        theme::dim(),
    ));
    Ok(shown)
}

fn basis_words(basis: Basis) -> &'static str {
    match basis {
        Basis::List => "list prices",
        Basis::Net => "after fees",
        Basis::Instant => "sold now",
    }
}

/// A line of the games' list: a game, or the heading over this session's.
enum Item<'a> {
    Game(&'a MarketRow),
    Heading(usize),
}

/// The market, `w` wide and `room` rows: the banner, the games' table from
/// `top` (moved so game `at` shows) with the legend beside or below it; and
/// the line game `at` is on.
fn lines(
    view: &MarketView,
    at: usize,
    top: &mut usize,
    currency: Currency,
    w: usize,
    room: usize,
) -> Fits<(Vec<Line<'static>>, Option<usize>)> {
    let mut out = Vec::new();
    if let Some(banner) = &view.pause {
        out.extend(banner_lines(view, banner, w)?);
        out.push(Line::default());
    }
    let wide = w >= WIDE;
    let columns: Vec<(&str, usize)> = if wide {
        COLUMNS.to_vec()
    } else if w >= 52 {
        vec![
            ("GAME", w.saturating_sub(38).max(14)),
            ("CARDS", 12),
            ("≈ LEFT", 8),
            ("PRICED", 10),
        ]
    } else {
        vec![
            ("GAME", w.saturating_sub(18)),
            ("≈ LEFT", 8),
            ("PRICED", 10),
        ]
    };
    let list_w: usize = columns.iter().map(|c| c.1).sum();
    let head: String = columns
        .iter()
        .enumerate()
        .map(|(i, &(name, cw))| {
            if i == 0 {
                format!("{name:<cw$}")
            } else {
                format!("{name:>cw$}")
            }
        })
        .collect();
    let mut items: Vec<Item<'_>> = view.rows.iter().map(Item::Game).collect();
    if !view.session_rows.is_empty() {
        items.push(Item::Heading(view.session_rows.len()));
        items.extend(view.session_rows.iter().map(Item::Game));
    }
    let shows = if wide {
        room.saturating_sub(out.len() + 2).max(1)
    } else {
        NARROW_ROWS
    };
    // The chosen game's line among the items, headings and all.
    let chosen_item = items
        .iter()
        .enumerate()
        .filter(|(_, it)| matches!(it, Item::Game(_)))
        .nth(at)
        .map_or(0, |(i, _)| i);
    if chosen_item < *top {
        *top = chosen_item;
    } else if chosen_item >= *top + shows {
        *top = chosen_item + 1 - shows;
    }
    *top = (*top).min(items.len().saturating_sub(shows));
    let games = |its: &[Item<'_>]| its.iter().filter(|it| matches!(it, Item::Game(_))).count();
    let end = (*top + shows).min(items.len());
    // This session's games, when none of them shows yet, are told of as
    // following, not counted.
    let heading = items.iter().position(|it| matches!(it, Item::Heading(_)));
    let session_below = heading.is_some_and(|at| at >= end);
    let counted = if session_below {
        heading.unwrap_or(items.len())
    } else {
        items.len()
    };
    let above = games(&items[..*top]);
    let below = games(&items[end.min(counted)..counted]);

    let mut left = vec![Line::styled(head, theme::dim())];
    let mut focus = None;
    for (i, item) in items.iter().enumerate().take(end).skip(*top) {
        let line = match item {
            Item::Game(row) => game_row(view, row, &columns)?,
            Item::Heading(n) => rule(
                Line::styled(
                    format!("WITH CARDS THIS SESSION · {}", format::games(*n)),
                    theme::heading(),
                ),
                list_w,
                None,
            )?,
        };
        if i == chosen_item {
            focus = Some(left.len());
            left.push(chosen(fit(line, list_w)?));
        } else {
            left.push(line);
        }
    }
    let more = more_words(
        above,
        below,
        session_below.then_some(view.session_rows.len()),
    );
    if !more.is_empty() {
        left.push(first_fit(more, if wide { list_w } else { w })?);
    }
    let focus = focus.map(|f| out.len() + f);
    let side = side(view, currency, if wide { SIDE } else { w })?;
    if wide {
        for i in 0..left.len().max(side.len()) {
            let a = left.get(i).cloned().unwrap_or_default();
            let b = side.get(i).cloned().unwrap_or_default();
            out.push(join([
                fit(a, list_w)?,
                Line::styled(" │ ", theme::dim()),
                b,
            ]));
        }
    } else {
        out.extend(left);
        out.push(Line::default());
        out.extend(side);
    }
    Ok((out, focus))
}

/// "39 more ↓ · the 6 games with cards this session follow", longest first.
fn more_words(above: usize, below: usize, session: Option<usize>) -> Vec<String> {
    let mut parts = Vec::new();
    if above > 0 {
        parts.push(format!("{above} more ↑"));
    }
    if below > 0 {
        parts.push(format!("{below} more ↓"));
    }
    if parts.is_empty() {
        return session.map_or_else(Vec::new, |n| {
            vec![format!(
                "the {} with cards this session follow ↓",
                format::games(n)
            )]
        });
    }
    let counts = parts.join(" · ");
    match session {
        Some(n) => vec![
            format!(
                "{counts} · the {} with cards this session follow",
                format::games(n)
            ),
            counts,
        ],
        None => vec![counts],
    }
}

/// "── ‖ Prices paused by Steam ──── until 17:41 · the wait is an hour now ─",
/// and why, in words.
fn banner_lines(view: &MarketView, b: &Banner, w: usize) -> Fits<Vec<Line<'static>>> {
    let at = |t| format::clock(t, view.now, view.zone);
    let until = at(b.until);
    let wait = wait_words(b.step);
    let mut out = vec![rule_ladder(
        Line::styled("‖ Prices paused by Steam", theme::fg(theme::BUSY)),
        w,
        [
            Line::styled(
                format!("until {until} · the wait is {wait} now"),
                theme::dim(),
            ),
            Line::styled(format!("until {until}"), theme::dim()),
        ],
    )?];
    let when = b
        .since
        .map_or_else(String::new, |t| format!(" at {}", at(t)));
    let again = if b.step > FIRST_WAIT {
        ", and each try since,"
    } else {
        ","
    };
    out.extend(wrap(
        format!(
            "Steam turned down a price lookup{when}{again} so lookups wait: 10 minutes, then \
             20, 40, and an hour at most. The next try is at {until}. Farming carries on as \
             normal; only prices wait. Prices older than 6 hours show dim, with their age, and \
             still count."
        ),
        w,
        0,
    )?);
    Ok(out)
}

/// The first wait after Steam turns a lookup down.
const FIRST_WAIT: Duration = Duration::from_secs(10 * 60);

/// "an hour", "20 minutes".
fn wait_words(d: Duration) -> String {
    let minutes = d.as_secs() / 60;
    match minutes {
        60 => "an hour".to_owned(),
        m if m % 60 == 0 => format!("{} hours", m / 60),
        m => format!("{m} minutes"),
    }
}

/// "£0.04–0.06": a range, its currency written once.
fn range_words(range: Option<(Money, Money)>) -> Option<String> {
    let (lo, hi) = range?;
    let (lo_s, hi_s) = (format::money(lo), format::money(hi));
    if lo == hi {
        return Some(lo_s);
    }
    let symbol = lo.currency.symbol().unwrap_or("");
    if !symbol.is_empty() && hi_s.starts_with(symbol) {
        return Some(format!("{lo_s}–{}", hi_s[symbol.len()..].trim_start()));
    }
    if !symbol.is_empty() && lo_s.ends_with(symbol) {
        let lo_s = lo_s[..lo_s.len() - symbol.len()].trim_end();
        return Some(format!("{lo_s}–{hi_s}"));
    }
    Some(format!("{lo_s}–{hi_s}"))
}

/// A money cell: "£0.05", "…" on its way, "—" with no market, "?" failed.
fn money_words(cell: Option<Cell>) -> String {
    match cell {
        None => String::new(),
        Some(Cell::Value { value, .. }) => format::money(value),
        Some(Cell::Pending) => "…".to_owned(),
        Some(Cell::NoMarket | Cell::NotMarketable) => "—".to_owned(),
        Some(Cell::Failed) => "?".to_owned(),
        Some(Cell::Foreign(m)) => m.to_string(),
    }
}

/// A game's row: its name, its ranges, what a drop and its drops left are
/// worth, and when it was priced. A set over 6 hours old shows dim.
fn game_row(view: &MarketView, row: &MarketRow, columns: &[(&str, usize)]) -> Fits<Line<'static>> {
    let stale = matches!(row.priced, Priced::Stale(_));
    let style = if stale { theme::dim() } else { Style::new() };
    let range = |r: Option<(Money, Money)>| match range_words(r) {
        Some(t) => t,
        None if row.priced == Priced::Pending => "…".to_owned(),
        None => "—".to_owned(),
    };
    let priced = match row.priced {
        Priced::Fresh(age) => format::ago(age),
        Priced::Stale(age) => format!("stale {}", format::age(age)),
        Priced::Pending => "not yet".to_owned(),
        Priced::Failed(at) => format!("again {}", format::clock(at, view.now, view.zone)),
    };
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, &(name, w)) in columns.iter().enumerate() {
        let (text, text_style) = match name {
            "GAME" => (game_name(&row.name, w), Style::new()),
            "CARDS" => (range(row.normal), style),
            "FOILS" => (range(row.foil), style),
            "≈ A DROP" => (money_words(row.per_drop), style),
            "≈ LEFT" => (money_words(row.left), style),
            _ => (priced.clone(), theme::dim()),
        };
        // A figure is whole or absent: one too wide for its column goes.
        let text = if width(&text) <= w {
            text
        } else {
            String::new()
        };
        let cell = Line::styled(text, text_style);
        let cell = if i == 0 {
            fit(cell, w)?
        } else {
            rfit(cell, w)?
        };
        spans.extend(cell.spans);
    }
    Ok(Line::from(spans))
}

/// The currency's name in words, for the legend: "pounds".
fn currency_words(c: Currency) -> String {
    match c {
        Currency::GBP => "pounds".to_owned(),
        Currency::USD => "dollars".to_owned(),
        Currency::EUR => "euros".to_owned(),
        c => c.code().unwrap_or("the wallet's currency").to_owned(),
    }
}

/// The legend, `w` wide: what each price state means, and the cards this
/// session that aren't priced, and why.
fn side(view: &MarketView, currency: Currency, w: usize) -> Fits<Vec<Line<'static>>> {
    let sample = format::money(Money::new(5, currency));
    let other = if currency == Currency::USD {
        Currency::EUR
    } else {
        Currency::USD
    };
    let foreign = format::money(Money::new(7, other));
    let dim = theme::dim();
    let states: [(Line<'static>, String); 7] = [
        (Line::from(sample.clone()), "priced, fresh".to_owned()),
        (
            Line::styled(format!("{sample} 8h"), dim),
            "over 6h old: dim".to_owned(),
        ),
        (Line::styled("…", dim), "not looked up yet".to_owned()),
        (
            Line::styled("?", theme::fg(theme::BUSY)),
            "lookup failed: again after 24 hours".to_owned(),
        ),
        (
            Line::styled("no market", dim),
            "nobody is selling it".to_owned(),
        ),
        (Line::styled("—", dim), "can't be sold".to_owned()),
        (
            Line::from(foreign),
            format!(
                "not in {}: shown, left out of totals",
                currency_words(currency)
            ),
        ),
    ];
    let mut out = vec![Line::styled("WHAT PRICES SHOW", theme::heading())];
    for (mark, words) in states {
        for (i, l) in wrap(words, w.saturating_sub(MARK), 0)?
            .into_iter()
            .enumerate()
        {
            let lead = if i == 0 {
                fit(mark.clone(), MARK)?
            } else {
                Line::from(" ".repeat(MARK))
            };
            out.push(join([lead, l]));
        }
    }
    out.push(Line::default());
    let zero = format!("{}0", currency.symbol().unwrap_or(""));
    out.extend(wrap(
        format!("Unpriced cards never count as {zero}: totals say ≥ and how many."),
        w,
        0,
    )?);
    out.push(Line::default());
    let n = view.unpriced.len();
    out.push(Line::styled(
        if n == 0 {
            "UNPRICED THIS SESSION · none".to_owned()
        } else {
            format!("UNPRICED THIS SESSION · {n}")
        },
        theme::heading(),
    ));
    for u in &view.unpriced {
        out.push(unpriced(view, u, w)?);
    }
    Ok(out)
}

/// "Madeline · Celeste     … on its way": a card this session that isn't
/// priced, and why.
fn unpriced(view: &MarketView, u: &Unpriced, w: usize) -> Fits<Line<'static>> {
    let why = match u.why {
        Why::Pending => "… on its way",
        Why::NoMarket => "no market",
        Why::Failed => "? failed",
        Why::Foreign => "not converted",
        Why::Identifying => "not known yet",
        Why::Unknown => "couldn't tell",
    };
    let what = u
        .card
        .clone()
        .unwrap_or_else(|| format::clock(u.at, view.now, view.zone));
    let room = w.saturating_sub(width(why) + 1);
    let name = first_fit(
        [
            format!("{what} · {}", u.game),
            format!(
                "{what} · {}",
                game_name(&u.game, room.saturating_sub(width(&what) + 3).max(4))
            ),
            what.clone(),
        ],
        room,
    )?;
    spread(name, Line::styled(why, theme::dim()), w, 1)
}
