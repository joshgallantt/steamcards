// Help (docs/design/ui.md, mockup k): the whole key map of §4, the symbols,
// and the money marks, in three columns where they fit, then two, then one;
// it scrolls when it doesn't fit. While getting set up, only setting up's
// own keys work, so it shows those.

use ratatui::{
    layout::Rect,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx,
        text::{Fits, Hint, fit, join, key, wrap},
        theme,
    },
    Shown,
};

const KEYS: [Hint; 1] = [Hint::new("esc", "close", 0)];

/// The keys' column: a key's caps, then what it does.
const CAPS: usize = 13;
/// The three columns, and the gap between them.
const FIRST: usize = 40;
const SECOND: usize = 36;
const THIRD: usize = 34;
const GAP: usize = 2;

pub(super) fn shown(cx: &Ctx<'_>, area: Rect) -> Fits<Shown> {
    let w = usize::from(area.width).saturating_sub(6);
    let lines = if cx.app.onboarding.is_active() {
        setting_up(w)?
    } else {
        lines(w)?
    };
    Ok(Shown::new("Help", lines, &KEYS))
}

/// A heading.
fn head(t: &str) -> Line<'static> {
    Line::styled(t.to_owned(), theme::heading())
}

/// "[↑↓]          choose a game": the caps, then what they do.
fn keys(caps: &str, what: &str) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (i, k) in caps.split(' ').enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        spans.push(key(k));
    }
    let caps = fit(Line::from(spans.clone()), CAPS).unwrap_or_else(|_| Line::from(spans));
    join([caps, Line::from(format!(" {what}"))])
}

/// Help's three columns: the queue's keys and the views; the keys that work
/// everywhere, in a pop-up, and those kept for quick-sell; the symbols and
/// the money marks.
fn columns() -> [Vec<Line<'static>>; 3] {
    let queue = vec![
        head("THE QUEUE"),
        keys("↑↓", "choose a game"),
        keys("PgUp PgDn", "a page at a time"),
        keys("Home End", "first or last"),
        keys("enter", "the game's details"),
        keys("1-9", "rank it #1–#9"),
        keys("0", "back to indifferent"),
        Line::styled(
            format!("{}(Backspace, Delete too)", " ".repeat(CAPS + 1)),
            theme::dim(),
        ),
        keys("x", "skip it: never farmed"),
        keys("o", "open its card page"),
        keys("c", "show or hide done"),
        keys("t", "clock times, or countdowns"),
        Line::default(),
        head("VIEWS"),
        keys("h", "this session's cards"),
        keys("m", "the market: prices"),
        keys("g", "games & settings"),
        keys("a", "account"),
        keys("l", "log: every event"),
        keys("?", "this help"),
    ];
    let everywhere = vec![
        head("EVERYWHERE"),
        keys("p", "pause, or carry on"),
        keys("b", "list, net or instant"),
        keys("q", "quit (asks if farming)"),
        keys("ctrl-c", "quit at once"),
        Line::default(),
        head("IN A POP-UP OR VIEW"),
        keys("esc", "close it (q too)"),
        keys("↑↓ PgUp", "scroll, or choose"),
        Line::default(),
        head("KEPT FOR QUICK-SELL, LATER"),
        keys("s", "sell this card now"),
        keys("k", "keep it: never listed"),
        keys("u", "take it off sale"),
        keys("tab", "the market's tabs"),
    ];
    let symbol = |glyph: &'static str, style, what: &'static str| {
        vec![Span::styled(glyph, style), Span::raw(format!(" {what}"))]
    };
    let good = theme::fg(theme::GOOD);
    let busy = theme::fg(theme::BUSY);
    let bad = theme::fg(theme::BAD);
    let dim = theme::dim();
    let plain = ratatui::style::Style::new();
    let pair = |a: Vec<Span<'static>>, b: Vec<Span<'static>>| {
        let mut spans = a;
        spans.push(Span::raw("  "));
        spans.extend(b);
        Line::from(spans)
    };
    let symbols = vec![
        head("SYMBOLS"),
        pair(
            symbol("▶", good, "farming alone"),
            symbol("▷", good, "building hours"),
        ),
        pair(
            symbol("‖", busy, "waiting or paused"),
            symbol("✓", good, "done"),
        ),
        pair(
            symbol("✕", bad, "skipped, or failed"),
            symbol("#1", theme::strong(theme::BUSY), "priority"),
        ),
        Line::from(symbol("›", plain, "chosen game, a light-blue bar")),
        pair(
            symbol("●", plain, "a drop you had"),
            symbol("◆", good, "this session"),
        ),
        pair(
            symbol("★", theme::fg(theme::ACCENT), "a foil this session"),
            symbol("○", dim, "to come"),
        ),
        pair(
            symbol("×2", plain, "two of that card"),
            symbol("—", dim, "none of it"),
        ),
        Line::default(),
        head("MONEY AND ESTIMATES"),
        Line::from(symbol("≈", plain, "an estimate")),
        Line::from(symbol("≥", plain, "at least: some aren't priced")),
        pair(
            symbol("…", dim, "not priced yet"),
            symbol("?", busy, "lookup failed"),
        ),
        Line::from(symbol("—", dim, "can't be sold")),
        Line::from(vec![
            Span::styled("no market", dim),
            Span::raw(": nobody is selling it"),
        ]),
        Line::from(symbol("8h", dim, "a price that old (shown dim)")),
        Line::from("list: what buyers pay"),
        Line::from("net: what you'd get, after fees"),
        Line::from("instant: what selling now gets"),
    ];
    [queue, everywhere, symbols]
}

/// How farming goes, under the keys.
const HOW: &str = "Cards drop for one game at a time, once it has 3 hours on record. Games \
                   short of that are played together, up to 32, to build hours. The time to \
                   finish learns from this session's drops.";

/// Help, `w` wide: three columns where they fit, then two, then one.
fn lines(w: usize) -> Fits<Vec<Line<'static>>> {
    let [a, b, c] = columns();
    let side_by_side = |cols: &[(&[Line<'static>], usize)]| -> Fits<Vec<Line<'static>>> {
        let rows = cols.iter().map(|c| c.0.len()).max().unwrap_or(0);
        (0..rows)
            .map(|i| {
                let mut row = Vec::new();
                for (n, (col, cw)) in cols.iter().enumerate() {
                    let cell = col.get(i).cloned().unwrap_or_default();
                    if n + 1 < cols.len() {
                        row.push(fit(cell, *cw)?);
                        row.push(Line::from(" ".repeat(GAP)));
                    } else {
                        row.push(cell);
                    }
                }
                Ok(join(row))
            })
            .collect()
    };
    let mut out = if w >= FIRST + SECOND + THIRD + 2 * GAP {
        let third = w - FIRST - SECOND - 2 * GAP;
        side_by_side(&[(&a, FIRST), (&b, SECOND), (&c, third)])?
    } else if w >= FIRST + GAP + THIRD {
        let mut left = a;
        left.push(Line::default());
        left.extend(b);
        side_by_side(&[(&left, FIRST), (&c, w - FIRST - GAP)])?
    } else {
        let mut all = a;
        for col in [b, c] {
            all.push(Line::default());
            all.extend(col);
        }
        all
    };
    out.push(Line::default());
    out.extend(wrap(HOW, w, 0)?);
    Ok(out)
}

/// Help while getting set up: setting up's keys, and how farming goes.
fn setting_up(w: usize) -> Fits<Vec<Line<'static>>> {
    let mut out = vec![
        head("SETTING UP"),
        keys("↑↓", "choose"),
        keys("enter", "go on, or sign in on the Steam row"),
        keys("→ ←", "the next step, or the one before"),
        keys("space", "pick or unpick a game"),
        keys("o", "only priority, on or off"),
        keys("r", "read your badges again"),
        keys("v", "appear offline or online"),
        keys("b", "list, net or instant (Start)"),
        keys("?", "this help"),
        keys("q", "quit"),
        keys("ctrl-c", "quit at once"),
        Line::default(),
    ];
    out.extend(wrap(HOW, w, 0)?);
    Ok(out)
}
