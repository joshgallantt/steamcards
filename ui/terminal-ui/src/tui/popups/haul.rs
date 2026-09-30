// This session's cards (docs/design/ui.md, mockup j): every copy that dropped,
// on its own row with its price and a running total that turns ≥ at the
// first card not priced; the day drawn as a track; the cards by game, with
// their spares; the session at each basis; the pace; and, in words, why each
// card that isn't priced isn't. o opens the chosen card's market page.

use market::{Basis, Held};
use ratatui::{
    layout::Rect,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx, format,
        layout::{
            progress::track,
            right::{card_words, price_words},
        },
        text::{Fits, Hint, first_fit, fit, join, rfit, width, wrap},
        theme,
    },
    Scroll, Shown, chosen, game_name, listed,
};
use crate::viewmodel::{Cell, GameState, Haul, HaulRow, Told, Unpriced, Why};

/// The chosen card, by when it dropped: the newest until the arrows move
/// it, when the pop-up scrolls to keep it in view. And how far the pop-up
/// is scrolled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in super::super) struct HaulView {
    pub(super) card: Option<usize>,
    pub(super) scroll: Scroll,
}

const TIME: usize = 5;
const GAME: usize = 16;
const CARD: usize = 22;
const PRICE: usize = 9;
const TOTAL: usize = 8;
/// The table: time, game, card, price and running total, 2 apart.
const TABLE: usize = TIME + 2 + GAME + 2 + CARD + 2 + PRICE + 2 + TOTAL;
/// The column beside the table, by game, needs this much.
const SIDE: usize = 40;
/// The track needs this much.
const TRACK: usize = 60;

const KEYS: [Hint; 4] = [
    Hint::new("↑↓", "choose", 1),
    Hint::new("b", "list/net/instant", 2),
    Hint::new("o", "market page", 3),
    Hint::new("esc", "close", 0),
];

pub(super) fn shown(cx: &Ctx<'_>, area: Rect, v: &HaulView) -> Fits<Shown> {
    let w = usize::from(area.width);
    let inner = w.saturating_sub(6);
    let h = Haul::build(&cx.s);
    let chosen = v.card.unwrap_or(h.rows.len().saturating_sub(1));
    let (lines, focus) = lines(&h, chosen, cx.spinner, inner)?;
    let mut shown = Shown::new("This session's cards", lines, &KEYS);
    // Opened, it shows from the top, the track and the column names in
    // view; once the arrows choose a card, it follows it.
    shown.focus = focus.filter(|_| v.card.is_some());
    shown.note = first_fit(note(&h), w.saturating_sub(30)).ok();
    Ok(shown)
}

/// How values are shown, as the haul's note says it.
fn basis_words(basis: Basis) -> &'static str {
    match basis {
        Basis::List => "list prices",
        Basis::Net => "after fees",
        Basis::Instant => "sold now",
    }
}

/// The price column's name: "LIST", "NET", "INSTANT".
fn column(basis: Basis) -> &'static str {
    match basis {
        Basis::List => "LIST",
        Basis::Net => "NET",
        Basis::Instant => "INSTANT",
    }
}

/// The note in the top border, longest first: "16 cards · ≥ £1.45 · 3
/// unpriced · list prices".
fn note(h: &Haul) -> Vec<Line<'static>> {
    if h.rows.is_empty() {
        return vec![Line::styled("no cards yet", theme::dim())];
    }
    let cards = format::cards(h.rows.len());
    let Some(total) = &h.total else {
        return vec![Line::styled(cards, theme::dim())];
    };
    let held = format::held(total);
    vec![
        Line::styled(
            format!("{cards} · {held} · {}", basis_words(h.basis)),
            theme::dim(),
        ),
        Line::styled(format!("{cards} · {held}"), theme::dim()),
    ]
}

/// The running total after a row: "£0.27", "≥ £1.30".
fn total_words(r: &HaulRow) -> String {
    r.total.map_or_else(String::new, |t| {
        if r.at_least {
            format!("≥ {}", format::money(t))
        } else {
            format::money(t)
        }
    })
}

/// The table's columns when they stand side by side: the time (with its
/// day, over a session longer than a day), the game and the card. The game
/// gives way first: a card's name is never shortened.
#[derive(Debug, Clone, Copy)]
struct Columns {
    time: usize,
    game: usize,
    card: usize,
}

impl Columns {
    /// The columns the rows need in `TABLE` columns, or `None` when the
    /// game would have under `GAME_MIN`.
    fn of(h: &Haul, spinner: &str) -> Option<Self> {
        let time = h
            .rows
            .iter()
            .map(|r| width(&format::clock(r.at, h.now, h.zone)))
            .max()
            .unwrap_or(TIME)
            .max(TIME);
        let card = h
            .rows
            .iter()
            .map(|r| card_words(&r.card, r.copy, spinner, CARD).width())
            .max()
            .unwrap_or(CARD)
            .max(CARD);
        let game = TABLE.checked_sub(time + 2 + 2 + card + 2 + PRICE + 2 + TOTAL)?;
        (game >= GAME_MIN).then_some(Self { time, game, card })
    }
}

/// The game's column is never narrower than this beside the others.
const GAME_MIN: usize = 8;

/// The haul, `w` wide, card `chosen` marked; and the line it's on.
fn lines(
    h: &Haul,
    chosen_card: usize,
    spinner: &str,
    w: usize,
) -> Fits<(Vec<Line<'static>>, Option<usize>)> {
    if h.rows.is_empty() {
        let lines = wrap(
            "No cards yet. The first usually drops within about half an hour of a game being \
             played on its own, and shows here a few seconds later, with its price.",
            w,
            0,
        )?;
        return Ok((lines, None));
    }
    let mut out = if w >= TRACK {
        track(&h.track, w, 2, h.zone)?
    } else {
        Vec::new()
    };
    let cols = Columns::of(h, spinner).filter(|_| w >= TABLE);
    let table_w = if cols.is_some() { TABLE } else { w };
    let mut table = vec![head(h, cols, w)?];
    let mut focus = None;
    for (i, r) in h.rows.iter().enumerate() {
        let lines = row(h, r, spinner, cols, w)?;
        if i == chosen_card.min(h.rows.len() - 1) {
            focus = Some(out.len() + table.len());
            for l in lines {
                table.push(chosen(fit(l, table_w)?));
            }
        } else {
            table.extend(lines);
        }
    }
    let side = side(h)?;
    if w.saturating_sub(table_w + 3) >= SIDE {
        for i in 0..table.len().max(side.len()) {
            let a = table.get(i).cloned().unwrap_or_default();
            let b = side.get(i).cloned().unwrap_or_default();
            out.push(join([
                fit(a, table_w)?,
                Line::styled(" │ ", theme::dim()),
                b,
            ]));
        }
    } else {
        out.extend(table);
        out.push(Line::default());
        out.extend(side);
    }
    out.push(Line::default());
    out.extend(wrap(said(h), w, 0)?);
    Ok((out, focus))
}

/// The table's column names, as wide as its rows.
fn head(h: &Haul, cols: Option<Columns>, w: usize) -> Fits<Line<'static>> {
    let price = column(h.basis);
    let text = match cols {
        Some(Columns { time, game, card }) => format!(
            "{:<time$}  {:<game$}  {:<card$}  {price:>PRICE$}  {:>TOTAL$}",
            "TIME", "GAME", "CARD", "TOTAL"
        ),
        None => {
            let room = w.saturating_sub(2 * PRICE);
            format!(
                "{:<room$}{price:>PRICE$}{:>PRICE$}",
                "TIME  CARD · GAME", "TOTAL"
            )
        }
    };
    fit(
        Line::styled(text, theme::dim()),
        if cols.is_some() { TABLE } else { w },
    )
}

/// A card's row: its time, game and card, its price and the running total.
/// Narrow, a card whose words don't leave room for the rest takes a line of
/// its own, and its game, price and total the next.
fn row(
    h: &Haul,
    r: &HaulRow,
    spinner: &str,
    cols: Option<Columns>,
    w: usize,
) -> Fits<Vec<Line<'static>>> {
    let time = Line::styled(format::clock(r.at, h.now, h.zone), theme::dim());
    let price = price_words(r.price.or(Some(Cell::Pending)));
    let total = Line::from(total_words(r));
    if let Some(cols) = cols {
        return Ok(vec![join([
            fit(time, cols.time)?,
            Line::from("  "),
            fit(Line::from(game_name(&r.game, cols.game)), cols.game)?,
            Line::from("  "),
            fit(card_words(&r.card, r.copy, spinner, cols.card), cols.card)?,
            Line::from("  "),
            rfit(price, PRICE)?,
            Line::from("  "),
            rfit(total, TOTAL)?,
        ])]);
    }
    let lead = time.width() + 1;
    let room = w.saturating_sub(lead + 2 * PRICE);
    let c = card_words(&r.card, r.copy, spinner, room.saturating_sub(3 + 6));
    let cw = c.width();
    let what = first_fit(
        [
            join([c.clone(), Line::from(format!(" · {}", r.game))]),
            join([
                c.clone(),
                Line::from(format!(
                    " · {}",
                    game_name(&r.game, room.saturating_sub(cw + 3).max(4))
                )),
            ]),
            c.clone(),
        ],
        room,
    );
    if let Ok(what) = what {
        return Ok(vec![join([
            time,
            Line::from(" "),
            fit(what, room)?,
            rfit(price, PRICE)?,
            rfit(total, PRICE)?,
        ])]);
    }
    let mut out: Vec<Line<'static>> = wrap(join([time, Line::from(" "), c]), w, lead)?;
    let game = Line::from(format!(
        "{}· {}",
        " ".repeat(lead),
        game_name(&r.game, room.saturating_sub(2))
    ));
    out.push(join([
        fit(game, w.saturating_sub(2 * PRICE))?,
        rfit(price, PRICE)?,
        rfit(total, PRICE)?,
    ]));
    Ok(out)
}

/// "≥ £1.45", or "…" for a value not known.
fn at_least(held: Option<&Held>) -> String {
    held.map_or_else(|| "…".to_owned(), format::at_least)
}

/// The column beside the table: the cards by game, the session at each
/// basis, and the pace.
fn side(h: &Haul) -> Fits<Vec<Line<'static>>> {
    let dim = |t: String| Line::styled(t, theme::dim());
    let price = column(h.basis);
    let mut out = vec![dim(format!(
        "{:<16}{:>6}{:>6}{price:>9}",
        "BY GAME", "CARDS", "SPARE"
    ))];
    let spares = |n: u32| {
        if n == 0 {
            "—".to_owned()
        } else {
            n.to_string()
        }
    };
    for g in &h.by_game {
        let glyph = match g.state {
            GameState::Done => Span::styled("✓", theme::fg(theme::GOOD)),
            GameState::Farming => Span::styled("▶", theme::fg(theme::GOOD)),
            GameState::Other => Span::raw(" "),
        };
        out.push(join([
            fit(
                Line::from(vec![
                    glyph,
                    Span::raw(format!(" {}", game_name(&g.name, 14))),
                ]),
                16,
            )?,
            rfit(Line::from(g.cards.to_string()), 6)?,
            rfit(Line::from(spares(g.spares)), 6)?,
            rfit(Line::from(at_least(g.value.as_ref())), 9)?,
        ]));
    }
    out.push(join([
        Line::from(" ".repeat(16)),
        rfit(Line::from(h.rows.len().to_string()), 6)?,
        rfit(Line::from(spares(h.spares)), 6)?,
        rfit(Line::styled(at_least(h.total.as_ref()), theme::bold()), 9)?,
    ]));
    if let Some(at_each) = &h.at_each {
        out.push(Line::default());
        out.push(Line::styled("AT EACH BASIS", theme::heading()));
        for (basis, held) in at_each {
            let (word, what) = match basis {
                Basis::List => ("list", "what buyers pay"),
                Basis::Net => ("net", "after Steam's fees"),
                Basis::Instant => ("instant", "sold now, to offers"),
            };
            let style = if *basis == h.basis {
                theme::bold()
            } else {
                ratatui::style::Style::new()
            };
            out.push(join([
                fit(Line::styled(word, style), 9)?,
                rfit(Line::styled(format::at_least(held), style), 9)?,
                dim(format!("  {what}")),
            ]));
        }
    }
    let pace = pace(h);
    if !pace.is_empty() {
        out.push(Line::default());
        out.push(Line::styled("PACE", theme::heading()));
        out.extend(pace.into_iter().map(Line::from));
    }
    Ok(out)
}

/// "2.1 drops an hour, farming alone", "≥ £0.17 an hour so far, at list
/// prices", "best: ★ Thanatos (Hades), £0.62".
fn pace(h: &Haul) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(rate) = h.pace.rate {
        out.push(format!("{}, farming alone", format::rate(rate)));
    }
    if let Some(per_hour) = h.pace.per_hour {
        let least = if h.total.is_some_and(|t| t.unpriced > 0) {
            "≥ "
        } else {
            ""
        };
        let basis = match h.basis {
            Basis::List => "at list prices",
            Basis::Net => "after fees",
            Basis::Instant => "if sold now",
        };
        out.push(format!(
            "{least}{} an hour so far, {basis}",
            format::money(per_hour)
        ));
    }
    if let Some(best) = &h.pace.best {
        let star = if best.foil { "★ " } else { "" };
        out.push(format!(
            "best: {star}{} ({}), {}",
            best.name,
            best.game,
            format::money(best.value)
        ));
    }
    out
}

/// What the haul says in words: how many cards from how many games, the
/// foils and spares among them, and why each card that isn't priced isn't.
fn said(h: &Haul) -> String {
    let games = format::games(h.by_game.len());
    let mut text = format!("{} from {games}", format::cards(h.rows.len()));
    let mut parts = Vec::new();
    match h.foils {
        0 => {}
        1 => parts.push("1 is a foil".to_owned()),
        n => parts.push(format!("{n} are foils")),
    }
    if h.spares > 0 {
        let names: Vec<String> = h
            .rows
            .iter()
            .filter(|r| r.copy.is_some_and(|c| c > 1))
            .filter_map(|r| match &r.card {
                Told::Named { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();
        let copy = if h.rows.iter().all(|r| r.copy.is_none_or(|c| c <= 2)) {
            "a second copy"
        } else {
            "another copy"
        };
        let are = if h.spares == 1 {
            "1 is a spare".to_owned()
        } else {
            format!("{} are spares", h.spares)
        };
        parts.push(format!(
            "{are}, {copy} of a card you already had ({})",
            names.join(", ")
        ));
    }
    if parts.is_empty() {
        text.push('.');
    } else {
        text.push_str(&format!(": {}.", listed(&parts, true)));
    }
    let n = h.unpriced.len();
    if n > 0 {
        let why: Vec<String> = h.unpriced.iter().map(|u| why(h, u)).collect();
        let are = if n == 1 {
            "1 isn't priced yet".to_owned()
        } else {
            format!("{n} aren't priced yet")
        };
        text.push_str(&format!(
            " {are}, so totals read ≥: {}.",
            listed(&why, false)
        ));
    }
    text
}

/// Why a card isn't priced: "Madeline waits for its price".
fn why(h: &Haul, u: &Unpriced) -> String {
    let card = u.card.clone().unwrap_or_default();
    let at = format::clock(u.at, h.now, h.zone);
    match u.why {
        Why::Pending => format!("{card} waits for its price"),
        Why::NoMarket => format!("nobody is selling {card}"),
        Why::Failed => format!("{card}'s price couldn't be looked up"),
        Why::Foreign => format!("{card} is priced in another currency"),
        Why::Identifying => format!("the {at} card is still being identified"),
        Why::Unknown => format!("the {at} card couldn't be told"),
    }
}
