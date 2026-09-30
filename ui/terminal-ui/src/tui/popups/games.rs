// Games & settings (docs/design/ui.md, mockup l): picking priority games in
// order, as today, beside a column of settings: only priority, appear
// offline, the value basis with this session at each basis, so the choice
// is concrete, and quick-sell, marked later.

use market::Basis;
use ratatui::{
    layout::Rect,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx, format,
        text::{Fits, Hint, fit, join, key, spread, wrap},
        theme,
    },
    Scroll, Shown, chosen, game_name,
};
use crate::viewmodel::{GameRow, Values};

/// The game under the cursor, among the picked games and then the rest;
/// the first of the rest shown; and how far the pop-up is scrolled.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in super::super) struct GamesView {
    pub(in super::super) cursor: usize,
    top: usize,
    pub(super) scroll: Scroll,
}

impl GamesView {
    /// With the cursor on the `cursor`th game.
    #[cfg(test)]
    pub(in super::super) fn at(cursor: usize) -> Self {
        Self {
            cursor,
            ..Self::default()
        }
    }
}

/// The games' column.
const GAMES: usize = 52;
/// The settings' column needs this much beside the games.
const SETTINGS: usize = 41;
/// The settings' text is set to this measure.
const MEASURE: usize = 40;
/// A setting's name, before what it means.
const NAME: usize = 10;
/// The rest of the games shown when the settings go below them.
const NARROW_ROWS: usize = 8;

pub(super) fn shown(cx: &Ctx<'_>, area: Rect, v: &mut GamesView) -> Fits<Shown> {
    let w = usize::from(area.width).saturating_sub(6);
    let room = usize::from(area.height).saturating_sub(2);
    let rows = cx.app.games.rows(cx.s.library().games());
    let picked = rows.iter().filter(|r| r.rank.is_some()).count();
    let cursor = v.cursor.min(rows.len().saturating_sub(1));
    let on_a_pick = rows.get(cursor).is_some_and(|r| r.rank.is_some());
    let keys = [
        Hint::new("↑↓", "select", 2),
        Hint::new("space", if on_a_pick { "unpick" } else { "pick" }, 0),
        Hint::new("1-9", "rank", 1),
        Hint::new("0", "unrank", 3),
        Hint::new("r", "read again", 4),
        Hint::new("esc", "close", 0),
    ];
    let two = w >= GAMES + 3 + SETTINGS;
    let games_w = if two { GAMES } else { w.min(GAMES) };
    let (left, focus) = games(cx, &rows, cursor, &mut v.top, games_w, two.then_some(room))?;
    let right = settings(cx, w.min(if two { w - GAMES - 3 } else { w }))?;
    let lines = if two {
        (0..left.len().max(right.len()))
            .map(|i| {
                Ok(join([
                    fit(left.get(i).cloned().unwrap_or_default(), GAMES)?,
                    Line::styled(" │ ", theme::dim()),
                    right.get(i).cloned().unwrap_or_default(),
                ]))
            })
            .collect::<Fits<Vec<_>>>()?
    } else {
        let mut all = left;
        all.push(Line::default());
        all.extend(right);
        all
    };
    let mut shown = Shown::new("Games & settings", lines, &keys);
    shown.focus = focus;
    shown.note = Some(Line::styled(
        match picked {
            0 => "none picked".to_owned(),
            n => format!("{n} picked"),
        },
        theme::dim(),
    ));
    Ok(shown)
}

/// "#1  LIMBO                         2 to drop · 3.4h ".
fn row(r: &GameRow, chosen_row: bool, w: usize) -> Fits<Line<'static>> {
    let mark = if chosen_row { "›" } else { " " };
    let rank = match r.rank {
        Some(n) => Span::styled(
            format!("{:<4}", format!("#{n}")),
            theme::strong(theme::BUSY),
        ),
        None => Span::raw("    "),
    };
    let facts = Line::styled(
        format!(
            "{} to drop · {} ",
            r.drops.remaining,
            format::hours(r.hours)
        ),
        theme::dim(),
    );
    let name = game_name(&r.name, w.saturating_sub(24));
    let line = spread(
        Line::from(vec![Span::raw(mark), rank, Span::raw(name)]),
        facts,
        w,
        1,
    )?;
    Ok(if chosen_row { chosen(line) } else { line })
}

/// The games, `w` wide: the picked ones in their order, then the rest, as
/// many as `room` rows leave room for (a few when the settings go below),
/// from `top` and moved so the cursor shows; and the cursor's line.
fn games(
    cx: &Ctx<'_>,
    rows: &[GameRow],
    cursor: usize,
    top: &mut usize,
    w: usize,
    room: Option<usize>,
) -> Fits<(Vec<Line<'static>>, Option<usize>)> {
    let picked: Vec<&GameRow> = rows.iter().filter(|r| r.rank.is_some()).collect();
    let rest: Vec<&GameRow> = rows.iter().filter(|r| r.rank.is_none()).collect();
    let mut out = Vec::new();
    let mut focus = None;
    if picked.is_empty() {
        out.push(Line::from(vec![
            Span::styled("PRIORITY GAMES", theme::heading()),
            Span::styled(" · none picked: ", theme::dim()),
            key("space"),
            Span::styled(" picks one", theme::dim()),
        ]));
    } else {
        out.push(Line::from(vec![
            Span::styled("PRIORITY GAMES", theme::heading()),
            Span::styled(" · farmed first, in this order", theme::dim()),
        ]));
    }
    for (i, r) in picked.iter().enumerate() {
        if i == cursor {
            focus = Some(out.len());
        }
        out.push(row(r, i == cursor, w)?);
    }
    out.push(Line::default());
    out.push(Line::styled(
        format!("OTHER GAMES WITH CARDS TO DROP · {}", rest.len()),
        theme::heading(),
    ));
    if rows.is_empty() {
        out.extend(library_status(cx, w)?);
        return Ok((out, None));
    }
    // The rest shown, and the line saying how many more there are.
    let shows = match room {
        Some(room) => room.saturating_sub(out.len() + 1).max(1),
        None => NARROW_ROWS,
    };
    let shows = if rest.len() <= shows + 1 {
        rest.len()
    } else {
        shows
    };
    let at = cursor.checked_sub(picked.len());
    if let Some(at) = at {
        if at < *top {
            *top = at;
        } else if at >= *top + shows {
            *top = at + 1 - shows;
        }
    }
    *top = (*top).min(rest.len().saturating_sub(shows));
    for (i, r) in rest.iter().enumerate().skip(*top).take(shows) {
        let here = at == Some(i);
        if here {
            focus = Some(out.len());
        }
        out.push(row(r, here, w)?);
    }
    let (above, below) = (*top, rest.len().saturating_sub(*top + shows));
    let more: Vec<String> = [
        (above > 0).then(|| format!("{above} more ↑")),
        (below > 0).then(|| format!("{below} more ↓")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !more.is_empty() {
        out.push(Line::styled(more.join(" · "), theme::dim()));
    }
    Ok((out, focus))
}

/// Reading, failed or empty: the library, when it's not simply there.
fn library_status(cx: &Ctx<'_>, w: usize) -> Fits<Vec<Line<'static>>> {
    let library = &cx.app.library;
    if library.is_loading() {
        return Ok(vec![Line::from(vec![
            Span::styled(cx.spinner, theme::fg(theme::BUSY)),
            Span::raw(" Reading your badges for games with cards left"),
        ])]);
    }
    let text = match library.error() {
        Some(e) => format!("Couldn't read your badges: {e}. Press r to try again."),
        None => "No games with cards left to drop right now: steamcards looks again every few \
                 hours."
            .to_owned(),
    };
    wrap(Line::styled(text, theme::dim()), w, 0)
}

/// " ◉ Appear offline        [v]": a setting that's on or off, and its key.
fn toggle(on: bool, label: &str, k: &str) -> Fits<Line<'static>> {
    let mark = if on {
        Span::styled(" ◉ ", theme::strong(theme::GOOD))
    } else {
        Span::styled(" ○ ", theme::dim())
    };
    Ok(join([
        Line::from(mark),
        fit(Line::from(label.to_owned()), 22)?,
        Line::from(key(k)),
    ]))
}

/// What a setting means, under it: indented 3, at the column's measure.
fn meaning(text: &str, w: usize) -> Fits<Vec<Line<'static>>> {
    Ok(wrap(
        Line::styled(text.to_owned(), theme::dim()),
        w.saturating_sub(3),
        0,
    )?
    .into_iter()
    .map(|l| join([Line::from("   "), l]))
    .collect())
}

/// The settings' column, `w` wide.
fn settings(cx: &Ctx<'_>, w: usize) -> Fits<Vec<Line<'static>>> {
    let measure = w.min(MEASURE);
    let prefs = cx.s.prefs;
    let mut out = vec![Line::styled("FARMING", theme::heading())];
    out.push(toggle(prefs.only_priority, "Only priority", "o")?);
    out.extend(meaning(
        if prefs.only_priority {
            "On: only your priority games are farmed."
        } else {
            "Off: the rest follow, closest to dropping first."
        },
        measure,
    )?);
    out.push(toggle(!prefs.appear_online, "Appear offline", "v")?);
    out.extend(meaning(
        if prefs.appear_online {
            "Off: friends see you online, and every game being played."
        } else {
            "Friends don't see the games being played. Steam counts them the same."
        },
        measure,
    )?);
    out.push(Line::default());
    out.push(Line::from(vec![
        Span::styled("VALUES", theme::heading()),
        Span::raw("   "),
        key("b"),
    ]));
    let basis = cx.s.basis;
    for (b, name, what) in [
        (Basis::List, "List", "what buyers pay: the lowest listing"),
        (Basis::Net, "Net", "what you'd get after Steam's fees"),
        (
            Basis::Instant,
            "Instant",
            "what selling now gets: the best offer, after fees",
        ),
    ] {
        let on = b == basis;
        let mark = if on {
            Span::styled(" ◉ ", theme::strong(theme::GOOD))
        } else {
            Span::styled(" ○ ", theme::dim())
        };
        let lead = 3 + NAME;
        for (i, l) in wrap(
            Line::styled(what, theme::dim()),
            measure.saturating_sub(lead),
            0,
        )?
        .into_iter()
        .enumerate()
        {
            let head = if i == 0 {
                join([Line::from(mark.clone()), fit(Line::from(name), NAME)?])
            } else {
                Line::from(" ".repeat(lead))
            };
            out.push(join([head, l]));
        }
    }
    if let Some(v) = Values::build(&cx.s) {
        let each: Vec<String> = v
            .at_each
            .iter()
            .map(|(_, held)| format::at_least(held))
            .collect();
        let line = format!("This session: {}", each.join(" · "));
        if 3 + super::super::text::width(&line) <= w {
            out.push(Line::from(format!("   {line}")));
        } else {
            out.extend(meaning(&line, w)?);
        }
    }
    out.push(Line::default());
    out.push(Line::from(vec![
        Span::styled("SELLING", theme::heading()),
        Span::styled(" · later", theme::dim()),
        Span::raw("   "),
        key("m"),
    ]));
    out.extend(meaning(
        "Quick-sell will list each card as it drops, if you turn it on.",
        measure,
    )?);
    Ok(out)
}
