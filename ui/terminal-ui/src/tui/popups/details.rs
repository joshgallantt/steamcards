// A game's details (docs/design/ui.md, mockup i): everything about the chosen
// game. What's happening to it and why; its drops, and this session's; its
// value; the whole set, with a count per card and its normal, foil and (with
// room) sell-now prices; how many cards short of a badge; when it was
// priced; and its farm priority. ↑↓ moves to the next game without closing.

use market::Basis;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx, format,
        layout::right::{price_words, radio_rows},
        text::{Fits, Hint, first_fit, fit, join, key, pips, rfit, rule, rule_ladder, width, wrap},
        theme,
    },
    Shown, game_name, listed,
};
use crate::viewmodel::{Cell, ChosenGame, Doing, MOST_PIPS, SetCard, TheSet, Told};

/// The labels' width: "Drops  ", "Value  ".
const LABEL: usize = 7;
/// From this wide, the set's table has room for its sell-now prices.
const WIDE: usize = 64;
/// The narrowest a card's name gets in the set's table.
const CARD: usize = 12;

const KEYS: [Hint; 3] = [
    Hint::new("↑↓", "game", 1),
    Hint::new("o", "card page", 2),
    Hint::new("esc", "close", 0),
];

pub(super) fn shown(cx: &Ctx<'_>, area: Rect) -> Fits<Shown> {
    let w = usize::from(area.width);
    let inner = w.saturating_sub(6);
    let Some(g) = cx.app.selected.and_then(|id| ChosenGame::build(&cx.s, id)) else {
        let lines = vec![Line::styled("No game is chosen.", theme::dim())];
        return Ok(Shown::new("Details", lines, &KEYS[2..]));
    };
    let mut shown = Shown::new("", lines(&g, inner)?, &KEYS);
    shown.title = Line::styled(game_name(&g.name, inner), theme::heading());
    shown.note = (w >= 40).then(|| note(&g));
    Ok(shown)
}

/// What it's doing, in its title's border: "▶ farming alone".
fn note(g: &ChosenGame) -> Line<'static> {
    let (words, style) = match g.doing {
        Doing::Farming => ("▶ farming alone", theme::fg(theme::GOOD)),
        Doing::BuildingHours { .. } => ("▷ building hours", theme::fg(theme::GOOD)),
        Doing::Waiting => ("‖ waiting", theme::fg(theme::BUSY)),
        Doing::NextUp => ("next up", Style::new()),
        Doing::Queued => ("queued", theme::dim()),
        Doing::SetAside(_) => ("set aside", theme::fg(theme::BUSY)),
        Doing::Skipped => ("✕ skipped", theme::fg(theme::BAD)),
        Doing::NotFarmed => ("not farmed", theme::dim()),
        Doing::Done { .. } => ("✓ done", theme::fg(theme::GOOD)),
    };
    Line::styled(words, style)
}

/// The details of `g`, `w` wide.
fn lines(g: &ChosenGame, w: usize) -> Fits<Vec<Line<'static>>> {
    let mut out = vec![facts(g, w)?];
    out.extend(wrap(why(g), w, 2)?);
    out.push(Line::default());
    out.extend(drops(g, w)?);
    out.extend(value(g, w)?);
    out.push(Line::default());
    out.extend(set(g, w)?);
    out.push(Line::default());
    out.push(rule(
        Line::styled("Farm priority", theme::heading()),
        w,
        None,
    )?);
    out.extend(radio_rows(g, w)?);
    Ok(out)
}

/// "App 960910 · no badge yet · 4.0h on record · cards drop from 3h".
fn facts(g: &ChosenGame, w: usize) -> Fits<Line<'static>> {
    let badge = match g.badge_level {
        0 => "no badge yet".to_owned(),
        n => format!("badge level {n}"),
    };
    let head = format!("App {} · {badge}", g.app_id);
    let hours = format::hours(g.hours);
    let mut options = Vec::new();
    if g.remaining > 0 {
        options.push(format!("{head} · {hours} on record · cards drop from 3h"));
    }
    options.push(format!("{head} · {hours} on record"));
    options.push(format!("{head} · {hours}"));
    first_fit(options, w)
}

/// What's happening to it, and why: "▶ Farming now. Played on its own, so
/// its cards can drop. …".
fn why(g: &ChosenGame) -> Line<'static> {
    let glyph = |glyph: &'static str, style: Style, rest: String| {
        Line::from(vec![Span::styled(glyph, style), Span::raw(rest)])
    };
    let good = theme::fg(theme::GOOD);
    match g.doing {
        Doing::Farming => {
            let looks = match g.look_every {
                Some(every) => {
                    let minutes = every.as_secs() / 60;
                    let last = if g.remaining == 1 {
                        ", as it's the last card"
                    } else {
                        ""
                    };
                    format!(
                        " Its card page is looked at every {minutes} minutes{last}, and as soon \
                         as Steam says new items arrived."
                    )
                }
                None => " Its card page is looked at as soon as Steam says new items arrived."
                    .to_owned(),
            };
            glyph(
                "▶",
                good,
                format!(" Farming now. Played on its own, so its cards can drop.{looks}"),
            )
        }
        Doing::BuildingHours { others } => {
            let with = match others {
                0 => "on its own".to_owned(),
                1 => "with 1 other".to_owned(),
                n => format!("with {n} others"),
            };
            glyph(
                "▷",
                good,
                format!(
                    " Building hours, played {with}: cards drop only once a game has 3 hours \
                     on record. It needs {:.1}h more.",
                    g.hours_to_go
                ),
            )
        }
        Doing::Waiting => glyph(
            "‖",
            theme::fg(theme::BUSY),
            " Farmed next, once the game played on another device stops: farming waits for it."
                .to_owned(),
        ),
        Doing::NextUp => Line::from("Next up: it's farmed first when farming carries on."),
        Doing::Queued if g.hours_to_go > 0.0 => Line::from(format!(
            "Queued: it needs {:.1}h more on record before its cards can drop, so it builds \
             hours with the other games short of 3 hours.",
            g.hours_to_go
        )),
        Doing::Queued => Line::from("Queued: farmed in its turn, after the games above it."),
        Doing::SetAside(aside) if aside.times > 1 => Line::from(
            "No card in 10 hours of play, twice, so it's left alone for the rest of the session.",
        ),
        Doing::SetAside(_) => {
            Line::from("No card in 10 hours of play, so it went behind the others.")
        }
        Doing::Skipped => Line::from(vec![
            Span::styled("✕", theme::fg(theme::BAD)),
            Span::raw(" Skipped: it's never farmed. "),
            key("0"),
            Span::raw(" farms it after your priorities, and "),
            key("1-9"),
            Span::raw(" ranks it."),
        ]),
        Doing::NotFarmed => Line::from(
            "Not farmed: \"only priority\" is on, and it isn't one of your priority games.",
        ),
        Doing::Done { at } => {
            let when = at.map_or_else(String::new, |t| {
                format!(", the last at {}", format::clock(t, g.now, g.zone))
            });
            glyph("✓", good, format!(" Every card has dropped{when}."))
        }
    }
}

/// "Drops  ●◆◆○ 3 of 4 · 1 to go · done in ≈ 25m, around 17:55", and this
/// session's drops of it.
fn drops(g: &ChosenGame, w: usize) -> Fits<Vec<Line<'static>>> {
    let p = &g.pips;
    let counts = if g.remaining > 0 {
        format!(
            "{} · {}",
            format::of(g.received, g.total),
            format::to_go(g.remaining)
        )
    } else {
        format!("{} · all dropped", format::of(g.received, g.total))
    };
    let with_pips = p.total <= MOST_PIPS;
    let head = |rest: String| {
        if with_pips {
            join([
                pips(p.before, p.today, p.foils, p.total),
                Line::from(format!(" {rest}")),
            ])
        } else {
            Line::from(rest)
        }
    };
    let mut options = Vec::new();
    if let Some(d) = g.done_in.filter(|_| g.remaining > 0) {
        let about = format::estimate(d);
        let at = format::estimate_at(g.now, d, g.zone);
        options.push(head(format!("{counts} · done in ≈ {about}, around {at}")));
        options.push(head(format!("{counts} · done in ≈ {about}")));
        options.push(head(format!("{counts} · ≈ {about}")));
    }
    options.push(head(counts));
    let room = w.saturating_sub(LABEL);
    let mut out = vec![join([
        fit(Line::from("Drops"), LABEL)?,
        first_fit(options, room)?,
    ])];
    if !g.this_session.is_empty() {
        let at = |t| format::clock(t, g.now, g.zone);
        let each: Vec<String> = g
            .this_session
            .iter()
            .map(|c| match &c.card {
                Told::Named { name, foil: true } => format!("★ {name} (foil) at {}", at(c.at)),
                Told::Named { name, .. } => format!("{name} at {}", at(c.at)),
                Told::Identifying => format!("one at {} still being identified", at(c.at)),
                Told::Unknown => format!("one at {} that couldn't be told", at(c.at)),
            })
            .collect();
        let text = format!("{} this session: {}", each.len(), listed(&each, true));
        for l in wrap(text, room, 0)? {
            out.push(join([Line::from(" ".repeat(LABEL)), l]));
        }
    }
    Ok(out)
}

/// "Value  ≈ £0.05 a drop · ≈ £0.05 still to drop, excl. foils", and what
/// this session's cards of it are worth.
fn value(g: &ChosenGame, w: usize) -> Fits<Vec<Line<'static>>> {
    let room = w.saturating_sub(LABEL);
    let after = if g.basis == Basis::List {
        ""
    } else {
        " after fees"
    };
    let mut rows: Vec<Line<'static>> = Vec::new();
    match (g.per_drop, g.left) {
        (Some(Cell::Value { value: each, .. }), Some(Cell::Value { value: left, .. })) => {
            let (each, left) = (format::about(each), format::about(left));
            rows.push(first_fit(
                [
                    format!("{each} a drop · {left} still to drop{after}, excl. foils"),
                    format!("{each} a drop · {left} to come"),
                ],
                room,
            )?);
        }
        (Some(Cell::Pending), _) => rows.push(first_fit(
            ["its prices are on their way", "prices on their way"],
            room,
        )?),
        (Some(Cell::Failed), _) => rows.push(first_fit(
            [
                "its prices couldn't be looked up: tried again after 24 hours",
                "prices couldn't be looked up",
            ],
            room,
        )?),
        (Some(_), _) => rows.push(first_fit(
            ["nobody is selling its cards", "no market"],
            room,
        )?),
        (None, _) => {}
    }
    if let Some(held) = &g.value_this_session {
        let money = format::money(held.total);
        let n = held.unpriced;
        let options = match (held.priced, n) {
            (0, n) if n > 0 => vec![
                format!("{} this session, not priced yet", format::cards(n as usize)),
                format!("{n} unpriced"),
            ],
            (_, 0) => vec![format!("{money} this session"), format!("{money} so far")],
            (_, n) => vec![
                format!(
                    "{money} this session · {} not priced yet",
                    format::cards(n as usize)
                ),
                format!("{money} so far · {n} unpriced"),
            ],
        };
        rows.push(first_fit(options, room)?);
    }
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(i, l)| {
            let label = if i == 0 { "Value" } else { "" };
            join([Line::from(format!("{label:<LABEL$}")), l])
        })
        .collect())
}

/// The set: a count per card, and its prices; how many short of a badge;
/// and when it was priced. Or, before its card page is read, its range.
fn set(g: &ChosenGame, w: usize) -> Fits<Vec<Line<'static>>> {
    let dim = |t: &str| Line::styled(t.to_owned(), theme::dim());
    let indent = |l: Line<'static>| join([Line::from("  "), l]);
    let Some(set) = &g.set else {
        let mut out = vec![rule_ladder(
            Line::styled("The set · not read yet", theme::heading()),
            w,
            [dim("list prices")],
        )?];
        let range = |r: Option<(market::Money, market::Money)>| {
            r.map(|(a, b)| {
                if a == b {
                    format::money(a)
                } else {
                    format!("{} – {}", format::money(a), format::money(b))
                }
            })
        };
        let cards = range(g.normal_range).unwrap_or_else(|| "not priced yet".to_owned());
        let mut options = Vec::new();
        if let Some(foils) = range(g.foil_range) {
            options.push(format!("cards {cards} · foils {foils}"));
        }
        options.push(format!("cards {cards}"));
        out.push(indent(first_fit(options, w.saturating_sub(2))?));
        for l in wrap(
            "Its cards, and how many of each you have, show once its card page is read: \
             the farmer reads it as it farms the game.",
            w.saturating_sub(2),
            0,
        )? {
            out.push(indent(l));
        }
        return Ok(out);
    };
    let heading = Line::styled(
        format!(
            "The set · {}",
            format::the_set(set.have, set.cards.len(), set.spares)
        ),
        theme::heading(),
    );
    // A set is priced at one go, so a stale one says its age once, here.
    let hints = match g.stale.map(format::age) {
        Some(age) => [
            dim(&format!("a badge takes one of each · {age} old")),
            dim(&format!("{age} old")),
        ],
        None => [dim("a badge takes one of each"), dim("list prices")],
    };
    let mut out = vec![rule_ladder(heading, w, hints)?];
    out.extend(table(g, set, w)?);
    out.push(indent(short_of_a_badge(set, w.saturating_sub(2))?));
    out.push(indent(priced(g, w.saturating_sub(2))?));
    Ok(out)
}

/// "—" for none, dim; "×2".
fn count(owned: u32) -> Line<'static> {
    let style = if owned == 0 {
        theme::dim()
    } else {
        Style::new()
    };
    Line::styled(format::count(owned), style)
}

/// The set's table's columns: the card's name, and each price's, as wide as
/// their widest cell and a gap ("no market" widens its column).
struct Columns {
    name: usize,
    normal: usize,
    foil: usize,
    sell_now: Option<usize>,
}

impl Columns {
    fn width(&self) -> usize {
        2 + self.name + 6 + self.normal + self.foil + self.sell_now.unwrap_or(0)
    }
}

/// The set as a table: CARD, OWNED, NORMAL, FOIL and, with room, SELL NOW,
/// with a mark for a card that dropped today where there's room for it. A
/// card's name is never shortened: a set whose names don't fit a table has
/// a line a card.
fn table(g: &ChosenGame, set: &TheSet, w: usize) -> Fits<Vec<Line<'static>>> {
    let widest = |min: usize, cell: fn(&SetCard) -> Cell| {
        set.cards
            .iter()
            .map(|c| set_price(cell(c)).width() + 1)
            .max()
            .unwrap_or(0)
            .max(min)
    };
    let mut cols = Columns {
        name: set
            .cards
            .iter()
            .map(|c| width(&c.name))
            .max()
            .unwrap_or(0)
            .max(CARD),
        normal: widest(8, |c| c.normal),
        foil: widest(8, |c| c.foil),
        sell_now: Some(widest(10, |c| c.sell_now)),
    };
    if w < WIDE || cols.width() > w {
        cols.sell_now = None;
    }
    if cols.width() > w {
        return set
            .cards
            .iter()
            .map(|c| {
                first_fit(
                    [
                        join([
                            Line::from(format!("  {} ", c.name)),
                            count(c.owned),
                            Line::from(" · "),
                            set_price(c.normal),
                            Line::from(" · foil "),
                            set_price(c.foil),
                        ]),
                        join([Line::from(format!("  {} ", c.name)), count(c.owned)]),
                    ],
                    w,
                )
            })
            .collect();
    }
    let (name, normal, foil) = (cols.name, cols.normal, cols.foil);
    let mut head = format!(
        "  {:<name$}{:>6}{:>normal$}{:>foil$}",
        "CARD", "OWNED", "NORMAL", "FOIL"
    );
    if let Some(sell_now) = cols.sell_now {
        head.push_str(&format!("{:>sell_now$}", "SELL NOW"));
    }
    let mut out = vec![Line::styled(head, theme::dim())];
    for c in &set.cards {
        out.push(card_row(g, c, &cols, w)?);
    }
    Ok(out)
}

/// A set's price in its table: dim when stale, its age said in the set's
/// rule rather than in each cell.
fn set_price(cell: Cell) -> Line<'static> {
    match cell {
        Cell::Value {
            value,
            stale: Some(_),
        } => Line::styled(format::money(value), theme::dim()),
        cell => price_words(Some(cell)),
    }
}

fn card_row(g: &ChosenGame, c: &SetCard, cols: &Columns, w: usize) -> Fits<Line<'static>> {
    let mut row = join([
        Line::from("  "),
        fit(Line::from(c.name.clone()), cols.name)?,
        rfit(count(c.owned), 6)?,
        rfit(set_price(c.normal), cols.normal)?,
        rfit(set_price(c.foil), cols.foil)?,
    ]);
    if let Some(sell_now) = cols.sell_now {
        row = join([row, rfit(set_price(c.sell_now), sell_now)?]);
        if let Some(first) = c.today.first() {
            let many = match c.today.len() {
                1 => "one".to_owned(),
                n => n.to_string(),
            };
            let mark = Line::from(vec![
                Span::raw("   "),
                Span::styled("◆", theme::fg(theme::GOOD)),
                Span::styled(
                    format!(" {many} today, {}", format::clock(*first, g.now, g.zone)),
                    theme::dim(),
                ),
            ]);
            if row.width() + mark.width() <= w {
                row = join([row, mark]);
            }
        }
    }
    Ok(row)
}

/// "3 short of a badge: ≈ £0.15 to buy them, at list prices", as fits.
fn short_of_a_badge(set: &TheSet, w: usize) -> Fits<Line<'static>> {
    let n = set.missing;
    if n == 0 {
        return first_fit(["a full set: a badge can be crafted", "a full set"], w);
    }
    match set.missing_cost {
        Some(cost) => {
            let cost = format::about(cost);
            first_fit(
                [
                    format!("{n} short of a badge: {cost} to buy them, at list prices"),
                    format!("{n} short of a badge: {cost} to buy"),
                    format!("{n} short of a badge"),
                ],
                w,
            )
        }
        None => first_fit([format!("{n} short of a badge")], w),
    }
}

/// "Prices from the Steam market at 15:21, 2h ago.", as fits.
fn priced(g: &ChosenGame, w: usize) -> Fits<Line<'static>> {
    let Some(at) = g.priced_at else {
        return first_fit(
            [
                "Not priced yet: prices come a few games a minute.",
                "Not priced yet.",
            ],
            w,
        );
    };
    let ago = format::ago((g.now - at).to_std().unwrap_or_default());
    let clock = format::clock(at, g.now, g.zone);
    let long = if g.stale.is_some() {
        format!(
            "Prices from the Steam market at {clock}, {ago}: over 6 hours old, so they show dim."
        )
    } else {
        format!("Prices from the Steam market at {clock}, {ago}.")
    };
    first_fit([long, format!("Priced {ago}.")], w)
}
