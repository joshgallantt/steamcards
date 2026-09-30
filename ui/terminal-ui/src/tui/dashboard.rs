// The dashboard (docs/design/ui.md §2 and §3): the header; Progress; the Now
// panel at L; the farm queue; the chosen game and this session's cards in the
// right-hand column at M and L; the strip and the footer. Each region is drawn
// from its view model by its ladder in `layout`, at the window's size class,
// and written through `text::put`, so nothing is ever cut. It ports how the
// generator of the spec's mockups put the regions together, and holds no rule
// of its own: what a region says at a size is decided in `layout`.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Color,
    text::{Line, Span},
};

use super::{
    Ctx, format,
    layout::{
        Regions, SizeClass, header, progress,
        queue::{self, Row, RuleSays, Times},
        right, strip,
    },
    text::{
        Fits, Titles, cfit, first_fit, key, panel, plain, put, put_lines, shorten, titles_fit,
        width, wrap,
    },
    theme,
};
use crate::viewmodel::{Activity, ChosenGame, Haul, Header, Now, Progress, Snapshot};
use market::{Basis, Held};

/// The chosen game's panel at L: the right-hand column's rows under Now
/// that it keeps, this session's cards taking the rest.
const CHOSEN_L: u16 = 16;
/// The chosen game's panel at M while there's no game to show.
const CHOSEN_EMPTY_M: u16 = 8;

/// Draws the dashboard over the whole frame, in the regions the frame is
/// laid out in, and returns where the queue is scrolled to, for the next
/// frame to start from.
pub(super) fn render(buf: &mut Buffer, cx: &Ctx<'_>) -> usize {
    let (r, s, p, spinner) = (&cx.regions, &cx.s, cx.progress, cx.spinner);
    let now = Now::build(s);
    let w = usize::from(buf.area.width);
    put(
        buf,
        r.header.x,
        r.header.y,
        r.header.width,
        &header::header(w, &Header::build(s)),
    );
    // This session's cards, and the track Progress draws at M, are for the
    // sizes with a right-hand column.
    let haul = r.right.map(|_| Haul::build(s));
    draw_progress(buf, r, p, &now, haul.as_ref(), spinner);
    let offset = draw_queue(buf, r, cx, spinner);
    if let Some(area) = r.now {
        draw_now(buf, area, p, &now, spinner);
    }
    if let (Some(area), Some(haul)) = (r.right, &haul) {
        draw_right(buf, area, r, cx, s, haul, spinner);
    }
    put(
        buf,
        r.strip.x,
        r.strip.y,
        r.strip.width,
        &strip::strip(&cx.app.strip(s), p, w).unwrap_or_default(),
    );
    put(
        buf,
        r.footer.x,
        r.footer.y,
        r.footer.width,
        &drawn(strip::footer(r.class, &p.activity, cx.app.show_done, w)),
    );
    offset
}

/// What a ladder gives, or nothing when not even its last rung fits. A test
/// fails on that, as on any text that doesn't fit; the running app, which
/// must keep farming, draws what it can.
fn drawn<T: Default>(fits: Fits<T>) -> T {
    match fits {
        Ok(t) => t,
        Err(e) if cfg!(test) => panic!("nothing fits: {e}"),
        Err(_) => T::default(),
    }
}

/// A panel's inside: a row in from its top and bottom borders, and a column
/// of padding in from each side.
fn inside(area: Rect) -> Rect {
    Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    )
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

fn heading(s: impl Into<String>) -> Line<'static> {
    Line::styled(s.into(), theme::heading())
}

/// The room for a note on the right of a border whose title on the left is
/// `title` columns wide, in a panel `w` wide.
fn note_room(w: usize, title: usize) -> usize {
    w.saturating_sub(title + 8)
}

// ── Progress ─────────────────────────────────────────────────────────────────

/// Progress: a panel at L, M and S, its title's note in its top border; at
/// XS three bare rows.
fn draw_progress(
    buf: &mut Buffer,
    r: &Regions,
    p: &Progress,
    now: &Now,
    haul: Option<&Haul>,
    spinner: &str,
) {
    let area = r.progress;
    let rows = usize::from(r.progress_rows);
    let track = haul.map(|h| &h.track);
    if r.class == SizeClass::Xs {
        // A column in from each side, as a panel's text is.
        let w = usize::from(area.width).saturating_sub(1);
        let lines = drawn(progress::panel_rows(
            p, now, None, spinner, r.class, w, rows,
        ));
        put_lines(buf, area, &lines);
        return;
    }
    let w = usize::from(area.width);
    let title = heading("Progress");
    let note = progress::panel_title(p, r.class, note_room(w, title.width()));
    panel(
        buf,
        area,
        theme::border(),
        &Titles {
            top_left: Some(title),
            top_right: note,
            ..Titles::default()
        },
    );
    let lines = drawn(progress::panel_rows(
        p,
        now,
        track,
        spinner,
        r.class,
        w.saturating_sub(4),
        rows,
    ));
    put_lines(buf, inside(area), &lines);
}

// ── Now, at L ────────────────────────────────────────────────────────────────

/// The Now panel: its border says the state at a glance (§5.4), since when
/// in its top border, and how often the game's cards are looked at in its
/// bottom one.
fn draw_now(buf: &mut Buffer, area: Rect, p: &Progress, now: &Now, spinner: &str) {
    let (lines, since, every) = drawn(right::now_panel(
        p,
        now,
        spinner,
        usize::from(area.width).saturating_sub(4),
    ));
    panel(
        buf,
        area,
        theme::fg(now_colour(&p.activity)),
        &Titles {
            top_left: Some(heading("Now")),
            top_right: since.map(|t| Line::styled(t, theme::dim())),
            bottom_left: every.map(|t| Line::styled(t, theme::dim())),
            bottom_right: None,
        },
    );
    put_lines(buf, inside(area), &lines);
}

/// The Now panel's border: farming, waiting, stopped by an error, or idle.
fn now_colour(activity: &Activity) -> Color {
    match activity {
        Activity::Farming { .. } | Activity::BuildingHours { .. } => theme::GOOD,
        Activity::Waiting { .. } | Activity::Paused | Activity::Reading | Activity::Checking => {
            theme::BUSY
        }
        Activity::Expired { .. }
        | Activity::Reconnecting { .. }
        | Activity::Unreadable { .. }
        | Activity::TakenOver => theme::BAD,
        Activity::NothingToFarm { .. } | Activity::Stopped | Activity::SignedOut => theme::DIM,
    }
}

// ── The farm queue ───────────────────────────────────────────────────────────

/// The farm queue: its column header, its sections' rules and its games from
/// where it's scrolled, the chosen game's row a bar; what's to go in its top
/// border, and the legend and what's out of sight in its bottom one. At XS
/// the border names the columns and carries the library's drops. Returns
/// where it's scrolled to.
fn draw_queue(buf: &mut Buffer, r: &Regions, cx: &Ctx<'_>, spinner: &str) -> usize {
    let area = r.queue;
    let (p, q, app) = (cx.progress, cx.queue, cx.app);
    if q.is_empty() {
        draw_empty_queue(buf, area, &p.activity, spinner);
        return 0;
    }
    let w = usize::from(area.width);
    let pip_width = usize::try_from(q.pip_width).unwrap_or(usize::MAX);
    let cols = match queue::queue_cols(w.saturating_sub(4), pip_width) {
        Ok(cols) => cols,
        Err(e) if cfg!(test) => panic!("the queue's columns: {e}"),
        Err(_) => return app.queue_offset,
    };
    let xs = r.class == SizeClass::Xs;
    let says = RuleSays {
        value: !xs,
        hint: !xs,
        done_shown: app.show_done,
    };
    let times = Times {
        clock: app.clock_times,
        now: p.now,
        zone: p.zone,
    };
    let rows = drawn(queue::rows(q, &cols, times, says));
    let room = usize::from(r.queue_rows());
    let offset = scrolled(&rows, room, app.queue_offset, app.selected);
    let (shown, above, below) = queue::window(&rows, room, offset);
    let mut lines = Vec::new();
    if r.queue_header() {
        lines.push(drawn(queue::queue_head(&cols)));
    }
    lines.extend(shown.iter().map(|row| match row {
        Row::Game(id, l) if app.selected == Some(*id) => bar(l),
        Row::Rule(l) | Row::Game(_, l) => l.clone(),
    }));
    let more = queue::more(above, below);
    let titles = if xs {
        xs_queue_titles(p, cols.done_in, &more, w)
    } else {
        queue_titles(r.class, p, &more, w)
    };
    panel(buf, area, theme::border(), &titles);
    put_lines(buf, inside(area), &lines);
    offset
}

/// Where the queue is scrolled to: from where it was, only as far as keeps
/// the chosen game in view, with the rules just above it.
fn scrolled(rows: &[Row], room: usize, offset: usize, chosen: Option<u32>) -> usize {
    let mut offset = offset.min(rows.len().saturating_sub(room));
    let at = chosen.and_then(|id| {
        rows.iter()
            .position(|r| matches!(r, Row::Game(game, _) if *game == id))
    });
    if let Some(at) = at {
        let mut top = at;
        while top > 0 && matches!(rows[top - 1], Row::Rule(_)) {
            top -= 1;
        }
        if top < offset {
            offset = top;
        }
        if at >= offset + room {
            offset = at + 1 - room;
        }
    }
    offset
}

/// The chosen game's row as the selection bar: the whole row reversed in
/// the selection's colour, the only blue on screen.
fn bar(row: &Line<'static>) -> Line<'static> {
    Line::from(
        row.spans
            .iter()
            .map(|s| Span::styled(s.content.clone(), theme::selected()))
            .collect::<Vec<_>>(),
    )
    .style(theme::selected())
}

/// The queue's borders at L, M and S: what's to go, with the time to finish
/// at L while a game or a group is farmed; the legend, as much as fits, and
/// how many games are out of sight.
fn queue_titles(class: SizeClass, p: &Progress, more: &str, w: usize) -> Titles {
    let farming = matches!(
        p.activity,
        Activity::Farming { .. } | Activity::BuildingHours { .. }
    );
    let eta = p
        .eta
        .as_ref()
        .filter(|e| class == SizeClass::L && farming && !e.assumed)
        .map(|e| e.eta);
    let title = heading("Farm queue");
    let note = first_fit(
        queue::queue_title(p.to_go.games, p.to_go.drops, eta),
        note_room(w, title.width()),
    )
    .ok()
    .filter(|l| l.width() > 0);
    let legend = queue::legend(w.saturating_sub(6 + width(more) + 4));
    Titles {
        top_left: Some(title),
        top_right: note,
        bottom_left: (legend.width() > 0).then_some(legend),
        bottom_right: (!more.is_empty()).then(|| Line::styled(more.to_owned(), theme::dim())),
    }
}

/// The queue's borders at XS, which has no column header: what's to go by
/// its title, the columns' names on the right, and the library's drops, as
/// Progress has no border for them.
fn xs_queue_titles(p: &Progress, done_in: bool, more: &str, w: usize) -> Titles {
    let to_go = queue::queue_title(p.to_go.games, p.to_go.drops, None)
        .last()
        .map(plain)
        .filter(|t| !t.is_empty());
    let mut titles = Vec::new();
    if let Some(to_go) = to_go {
        titles.push(Line::from(vec![
            Span::styled("Farm queue", theme::heading()),
            dim(format!(" · {to_go}")),
        ]));
    }
    titles.push(heading("Farm queue"));
    let columns = if done_in {
        vec!["hours · drops · ≈ done in", "hours · drops"]
    } else {
        vec!["hours · drops"]
    };
    let idle = matches!(p.activity, Activity::NothingToFarm { .. });
    let library = format!(
        "library {} of {} drops",
        p.library.received, p.library.total
    );
    Titles {
        top_left: first_fit(titles, w.saturating_sub(30)).ok(),
        top_right: first_fit(
            columns.into_iter().map(|c| Line::styled(c, theme::dim())),
            w.saturating_sub(26),
        )
        .ok(),
        bottom_left: first_fit(
            [Line::styled(library, theme::dim())],
            w.saturating_sub(8 + width(more) + 4),
        )
        .ok()
        .filter(|_| !idle),
        bottom_right: (!more.is_empty()).then(|| Line::styled(more.to_owned(), theme::dim())),
    }
}

/// The queue with no games in it, in its middle: while the badges are read,
/// that they are; once they're read, that there are no games with cards.
fn draw_empty_queue(buf: &mut Buffer, area: Rect, activity: &Activity, spinner: &str) {
    panel(
        buf,
        area,
        theme::border(),
        &Titles {
            top_left: Some(heading("Farm queue")),
            ..Titles::default()
        },
    );
    let inner = inside(area);
    let w = usize::from(inner.width);
    let (first, then) = match activity {
        Activity::Reading | Activity::Checking => (
            Line::from(vec![
                Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                Span::raw(" Reading your badges"),
            ]),
            "Games with cards left show up here.",
        ),
        Activity::NothingToFarm { .. } => (
            Line::from("No games with trading cards."),
            "Games you own with cards show up here.",
        ),
        _ => (
            Line::from("Nothing read yet."),
            "Games with cards left show up here.",
        ),
    };
    let mid = (inner.height / 2).saturating_sub(1);
    put_lines(
        buf,
        Rect::new(inner.x, inner.y + mid, inner.width, inner.height - mid),
        &[
            drawn(cfit(first, w)),
            drawn(cfit(Line::styled(then, theme::dim()), w)),
        ],
    );
}

// ── The right-hand column ────────────────────────────────────────────────────

/// The right-hand column: the chosen game over this session's cards. At L it
/// sits under Now and the chosen game keeps 16 rows; at M the column is
/// shared as `right::split_m` says, or once nothing is left to farm as
/// `right::split_summary` does.
fn draw_right(
    buf: &mut Buffer,
    area: Rect,
    r: &Regions,
    cx: &Ctx<'_>,
    s: &Snapshot<'_>,
    haul: &Haul,
    spinner: &str,
) {
    let p = cx.progress;
    let w = usize::from(area.width).saturating_sub(4);
    let rows = usize::from(area.height);
    let chosen = cx.app.selected.and_then(|id| ChosenGame::build(s, id));
    let (height, with_haul) = match &chosen {
        Some(g) => {
            let forms = drawn(right::chosen_forms(g, w));
            let (i, height, with_haul) = match r.class {
                SizeClass::L => {
                    let room = usize::from(CHOSEN_L.min(area.height));
                    let i = forms
                        .iter()
                        .position(|f| f.len() + 2 <= room)
                        .unwrap_or(forms.len().saturating_sub(1));
                    (i, room, true)
                }
                _ if p.summary.is_some() => right::split_summary(&forms, rows),
                _ => right::split_m(&forms, rows),
            };
            let height = u16::try_from(height).unwrap_or(area.height);
            let top = Rect { height, ..area };
            draw_chosen(buf, top, g, forms.get(i).map_or(&[][..], Vec::as_slice));
            (height, with_haul)
        }
        None => {
            let height = match r.class {
                SizeClass::L => CHOSEN_L,
                _ => CHOSEN_EMPTY_M,
            }
            .min(area.height);
            draw_no_game(buf, Rect { height, ..area }, &p.activity);
            (height, true)
        }
    };
    if with_haul && height < area.height {
        let rest = Rect {
            y: area.y + height,
            height: area.height - height,
            ..area
        };
        draw_haul(buf, rest, r.class, haul, p, spinner);
    }
}

/// The chosen game's panel, in the selection's colour: its name and
/// "selected" in its top border, and the keys to see all of it in its
/// bottom one.
fn draw_chosen(buf: &mut Buffer, area: Rect, g: &ChosenGame, lines: &[Line<'static>]) {
    let w = usize::from(area.width);
    let (title, note) = chosen_titles(&g.name, w);
    let keys = first_fit(
        [
            Line::from(vec![
                key("enter"),
                dim(" everything about it   "),
                key("o"),
                dim(" card page"),
            ]),
            Line::from(vec![
                key("enter"),
                dim(" all   "),
                key("o"),
                dim(" card page"),
            ]),
        ],
        w.saturating_sub(6),
    )
    .ok();
    panel(
        buf,
        area,
        theme::fg(theme::SELECT),
        &Titles {
            top_left: title,
            top_right: note,
            bottom_left: keys,
            bottom_right: None,
        },
    );
    put_lines(buf, inside(area), lines);
}

/// The chosen game's name and "selected", as the border has room for: the
/// name whole, then alone, then shortened at a word boundary, as in the
/// queue; its details show it in full.
fn chosen_titles(name: &str, w: usize) -> (Option<Line<'static>>, Option<Line<'static>>) {
    let selected = Line::styled("selected", theme::dim());
    let whole = heading(name);
    if titles_fit(w, Some(&whole), Some(&selected)) {
        return (Some(whole), Some(selected));
    }
    if titles_fit(w, Some(&whole), None) {
        return (Some(whole), None);
    }
    (shorten(name, w.saturating_sub(6)).ok().map(heading), None)
}

/// The chosen game's panel with no game to show: while the badges are read,
/// what will show here once they are.
fn draw_no_game(buf: &mut Buffer, area: Rect, activity: &Activity) {
    let text = match activity {
        Activity::Reading | Activity::Checking => {
            "Nothing to show until the badges are read: a game's cards, prices and priority \
             appear here."
        }
        _ => "Nothing to show: a game's cards, prices and priority appear here once it's queued.",
    };
    let inner = inside(area);
    let lines = wrap(text, usize::from(inner.width), 0)
        .ok()
        .filter(|l| l.len() <= usize::from(inner.height))
        .unwrap_or_default();
    panel(
        buf,
        area,
        theme::fg(theme::SELECT),
        &Titles {
            top_left: Some(heading("The chosen game")),
            ..Titles::default()
        },
    );
    put_lines(buf, inner, &lines);
}

/// This session's cards: at L a table with a running total, the day's track
/// on top when there's room and the other bases below; at M the newest
/// cards, each with its game and price. Its borders count the cards, their
/// value, those out of sight, and the spares or those not priced.
fn draw_haul(
    buf: &mut Buffer,
    area: Rect,
    class: SizeClass,
    h: &Haul,
    p: &Progress,
    spinner: &str,
) {
    let w = usize::from(area.width);
    let rows = usize::from(area.height).saturating_sub(2);
    if h.rows.is_empty() {
        panel(
            buf,
            area,
            theme::border(),
            &Titles {
                top_left: Some(heading("This session · no cards yet")),
                bottom_left: Some(Line::from(vec![key("h"), dim(" all")])),
                ..Titles::default()
            },
        );
        put_lines(
            buf,
            inside(area),
            &no_cards_yet(h.basis, w.saturating_sub(4), rows),
        );
        return;
    }
    let n = h.rows.len();
    let at_l = class == SizeClass::L;
    let (lines, start) = if at_l {
        drawn(right::haul_rows_l(h, rows, spinner, w.saturating_sub(4)))
    } else {
        drawn(right::haul_rows_m(h, rows, spinner, w.saturating_sub(4)))
    };
    let oldest = h
        .total
        .and_then(|t| t.oldest)
        .map(|d| format!("oldest {}", format::age(d)));
    let titles = if at_l {
        let spares = p.values.as_ref().filter(|v| v.spares.0 > 0).map(|v| {
            format!(
                "{}, {}",
                format::spares(v.spares.0),
                format::at_least(&v.spares.1)
            )
        });
        Titles {
            top_left: Some(heading(format!("This session · {}", format::cards(n)))),
            top_right: h.total.as_ref().map(held_words),
            bottom_left: Some(if start > 0 {
                Line::from(vec![
                    dim(format!("{start} earlier ↑ · ")),
                    key("h"),
                    dim(" all"),
                ])
            } else {
                Line::from(vec![key("h"), dim(" every card, by game")])
            }),
            bottom_right: oldest.or(spares).map(|t| Line::styled(t, theme::dim())),
        }
    } else {
        let earlier = if start > 0 {
            first_fit(
                [
                    Line::from(vec![
                        dim(format!("{start} earlier ↑ · ")),
                        key("h"),
                        dim(" all"),
                    ]),
                    Line::styled(format!("{start} more ↑"), theme::dim()),
                ],
                w.saturating_sub(18),
            )
            .ok()
        } else {
            Some(Line::from(vec![key("h"), dim(" all")]))
        };
        let room = w.saturating_sub(6 + earlier.as_ref().map_or(0, Line::width) + 4);
        let unpriced = h
            .total
            .filter(|t| t.unpriced > 0)
            .map(|t| format!("{} unpriced", t.unpriced));
        let mut notes = Vec::new();
        match (&unpriced, &oldest) {
            (Some(u), Some(o)) => notes.extend([format!("{u} · {o}"), u.clone()]),
            (Some(u), None) => notes.push(u.clone()),
            (None, Some(o)) => notes.push(o.clone()),
            (None, None) => {}
        }
        Titles {
            top_left: Some(heading(format!("This session · {n}"))),
            top_right: h
                .total
                .map(|t| Line::styled(format::at_least(&t), theme::bold())),
            bottom_left: earlier,
            bottom_right: first_fit(
                notes.into_iter().map(|t| Line::styled(t, theme::dim())),
                room,
            )
            .ok(),
        }
    };
    panel(buf, area, theme::border(), &titles);
    put_lines(buf, inside(area), &lines);
}

/// "≥ £1.45 · 3 unpriced": the value bold, the count of cards not priced
/// in the colour of something waiting.
fn held_words(held: &Held) -> Line<'static> {
    let mut spans = vec![Span::styled(format::at_least(held), theme::bold())];
    if held.unpriced > 0 {
        spans.push(dim(" · "));
        spans.push(Span::styled(
            format!("{} unpriced", held.unpriced),
            theme::fg(theme::BUSY),
        ));
    }
    Line::from(spans)
}

/// This session's cards before the first: when the first usually drops, and
/// that its price follows; then, with room, what the prices are, `w` wide
/// and at most `rows` tall.
fn no_cards_yet(basis: Basis, w: usize, rows: usize) -> Vec<Line<'static>> {
    let first = "No cards yet. The first usually drops within about half an hour of a game \
                 being played on its own, and shows here a few seconds later, with its price.";
    let (what, then) = match basis {
        Basis::List => (
            "Prices are what buyers pay now.",
            "switches to what you'd get after Steam's fees, or by selling at once.",
        ),
        Basis::Net => (
            "Prices are what you'd get after Steam's fees.",
            "switches to what selling at once gets, or what buyers pay.",
        ),
        Basis::Instant => (
            "Prices are what selling at once gets.",
            "switches to what buyers pay, or what you'd get after Steam's fees.",
        ),
    };
    let note = Line::from(vec![
        Span::raw(format!("{what} ")),
        key("b"),
        Span::raw(format!(" {then}")),
    ]);
    let Some(mut lines) = [first, "No cards yet: the first shows here with its price."]
        .into_iter()
        .filter_map(|t| wrap(t, w, 0).ok())
        .find(|l| l.len() <= rows)
    else {
        return Vec::new();
    };
    if let Ok(note) = wrap(note, w, 0)
        && lines.len() + 5 <= rows
        && lines.len() + 1 + note.len() <= rows
    {
        lines.push(Line::default());
        lines.extend(note);
    }
    lines
}

#[cfg(test)]
mod tests {
    // The golden tests: each of the spec's dashboard mockups (§3), rendered
    // at its size from the spec's data set, reads exactly as it's drawn.

    use ratatui::text::Line;

    use super::{Row, scrolled};
    use crate::tui::{fixtures, golden};

    #[test]
    fn the_queue_scrolls_only_as_far_as_keeps_the_chosen_game_in_view() {
        // A rule, three games, a rule, three games.
        let rows: Vec<Row> = [
            None,
            Some(1),
            Some(2),
            Some(3),
            None,
            Some(4),
            Some(5),
            Some(6),
        ]
        .into_iter()
        .map(|id| match id {
            Some(id) => Row::Game(id, Line::default()),
            None => Row::Rule(Line::default()),
        })
        .collect();
        assert_eq!(scrolled(&rows, 3, 0, Some(1)), 0, "in view: it stays");
        assert_eq!(scrolled(&rows, 3, 0, Some(5)), 4, "below: just far enough");
        assert_eq!(
            scrolled(&rows, 3, 6, Some(4)),
            4,
            "above: with the rule just above it"
        );
        assert_eq!(
            scrolled(&rows, 3, usize::MAX, None),
            5,
            "at most to the end"
        );
        assert_eq!(scrolled(&rows, 10, 3, Some(6)), 0, "all in view");
    }

    fn golden(title: &str) {
        let mut app = fixtures::for_mockup(title).expect("a fixture for each mockup");
        golden::check(title, &mut app);
    }

    #[tokio::test]
    async fn farming_alone_at_l() {
        golden("dashboard, farming alone (L)");
    }

    #[tokio::test]
    async fn farming_alone_in_the_users_window() {
        golden("dashboard, farming alone, the user's window (L)");
    }

    #[tokio::test]
    async fn farming_alone_at_m_with_height_to_spare() {
        golden("dashboard, farming alone (M, tall)");
    }

    #[tokio::test]
    async fn farming_alone_at_m() {
        golden("dashboard, farming alone (M)");
    }

    #[tokio::test]
    async fn farming_alone_at_s() {
        golden("dashboard, farming alone (S)");
    }

    #[tokio::test]
    async fn farming_alone_at_xs() {
        golden("dashboard, farming alone (XS)");
    }

    #[tokio::test]
    async fn the_queue_scrolled_to_its_end() {
        golden("the queue scrolled to its end (S)");
    }

    #[tokio::test]
    async fn building_hours_with_the_group() {
        golden("building hours with the 12-game group");
    }

    #[tokio::test]
    async fn the_first_minutes_of_a_session() {
        golden("the first minutes of a session");
    }

    #[tokio::test]
    async fn reading_the_badges_at_m() {
        golden("reading the badges (M)");
    }

    #[tokio::test]
    async fn reading_the_badges_at_s() {
        golden("reading the badges (S)");
    }

    #[tokio::test]
    async fn reading_the_badges_at_xs() {
        golden("reading the badges (XS)");
    }

    #[tokio::test]
    async fn steam_played_on_another_device() {
        golden("Steam played on another device");
    }

    #[tokio::test]
    async fn nothing_left_to_farm_sums_the_session_up() {
        golden("nothing left to farm, with the session's summary");
    }

    #[tokio::test]
    async fn paused() {
        golden("paused");
    }

    #[tokio::test]
    async fn sign_in_expired() {
        golden("sign-in expired");
    }

    #[tokio::test]
    async fn connection_lost_and_retrying() {
        golden("connection lost, retrying");
    }

    #[tokio::test]
    async fn prices_paused_by_steam_with_some_stale() {
        golden("prices paused by Steam, some stale");
    }
}
