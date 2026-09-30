// The dashboard (docs/design/ui.md §2.1, mockups a–h), assembled from the
// layout's ladders: the header, Progress, the Now panel at L, the farm queue,
// the chosen game and this session's cards beside it at M and L, the strip
// and the footer. Every region is drawn whole or not at all: a region whose
// ladder has no rung that fits stays blank.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
};

use super::{
    Ctx,
    layout::{
        SizeClass, haul_rows_m,
        header::header,
        progress::{
            progress_l, progress_m, progress_s, progress_title, progress_title_s, progress_xs,
        },
        queue::{
            Row, RuleSays, Times, legend, more, queue_cols, queue_head, queue_title, rows, window,
        },
        right::{chosen_forms, haul_rows_l, now_panel, split_m},
        strip::{footer, strip},
    },
    text::{Titles, cfit, first_fit, key, panel, put, put_lines, wrap},
    theme,
};
use crate::viewmodel::{Activity, ChosenGame, Haul, Header, Now, Progress, Queue};

/// Draws the dashboard; returns how far the queue is scrolled, to keep.
pub(super) fn render(buf: &mut Buffer, cx: &Ctx<'_>, queue: &Queue, offset: usize) -> usize {
    let r = cx.regions;
    let s = &cx.s;
    let p = Progress::build(s, cx.app.estimated);
    let now = Now::build(s);
    let w = usize::from(r.header.width);
    put(
        buf,
        0,
        r.header.y,
        r.header.width,
        &header(w, &Header::build(s)),
    );
    progress(buf, cx, &p, &now);
    let offset = farm_queue(buf, cx, &p, queue, offset);
    if let Some(area) = r.now {
        now_box(buf, cx, &p, &now, area);
    }
    if let Some(area) = r.right {
        right(buf, cx, area);
    }
    if let Ok(line) = strip(&cx.app.strip(s), &p, w) {
        put(buf, 0, r.strip.y, r.strip.width, &line);
    }
    if let Ok(line) = footer(r.class, &p.activity, cx.app.show_done, w) {
        put(buf, 0, r.footer.y, r.footer.width, &line);
    }
    offset
}

/// A panel with dim rounded borders, its titles, and its lines one column
/// in from each side.
fn boxed(buf: &mut Buffer, area: Rect, border: Style, titles: &Titles, lines: &[Line<'static>]) {
    panel(buf, area, border, titles);
    let inside = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );
    put_lines(
        buf,
        inside,
        &lines[..lines.len().min(usize::from(inside.height))],
    );
}

fn title(text: &str) -> Option<Line<'static>> {
    Some(Line::styled(text.to_owned(), theme::heading()))
}

fn progress(buf: &mut Buffer, cx: &Ctx<'_>, p: &Progress, now: &Now) {
    let r = cx.regions;
    let area = r.progress;
    let w = usize::from(area.width);
    let rows = match r.class {
        SizeClass::L => progress_l(p, now, cx.spinner, w.saturating_sub(4)),
        SizeClass::M => {
            let track = Haul::build(&cx.s).track;
            progress_m(
                p,
                now,
                Some(&track),
                cx.spinner,
                w.saturating_sub(4),
                usize::from(r.progress_rows),
            )
        }
        SizeClass::S => progress_s(p, now, cx.spinner, w.saturating_sub(4)),
        SizeClass::Xs | SizeClass::TooSmall => {
            if let Ok(rows) = progress_xs(p, now, cx.spinner, w.saturating_sub(1)) {
                put_lines(buf, area, &rows);
            }
            return;
        }
    };
    let budget = w.saturating_sub(16);
    let right = if r.class == SizeClass::S {
        first_fit(progress_title_s(p), budget).ok()
    } else {
        first_fit(progress_title(p), budget).ok()
    };
    let titles = Titles {
        top_left: title("Progress"),
        top_right: right.filter(|l| l.width() > 0),
        ..Titles::default()
    };
    boxed(
        buf,
        area,
        theme::border(),
        &titles,
        &rows.unwrap_or_default(),
    );
}

/// The farm queue, scrolled from `offset` so the chosen game shows;
/// returns where it's scrolled to.
fn farm_queue(buf: &mut Buffer, cx: &Ctx<'_>, p: &Progress, q: &Queue, offset: usize) -> usize {
    let r = cx.regions;
    let area = r.queue;
    let w = usize::from(area.width);
    let inner = w.saturating_sub(4);
    if matches!(p.activity, Activity::Reading) {
        let mut lines = vec![Line::default(); usize::from(area.height).saturating_sub(2)];
        let mid = (lines.len() / 2).saturating_sub(1);
        for (i, text) in [
            Line::from(vec![
                Span::styled(cx.spinner, theme::fg(theme::BUSY)),
                Span::raw(" Reading your badges"),
            ]),
            Line::styled("Games with cards left show up here.", theme::dim()),
        ]
        .into_iter()
        .enumerate()
        {
            if let (Some(slot), Ok(line)) = (lines.get_mut(mid + i), cfit(text, inner)) {
                *slot = line;
            }
        }
        let titles = Titles {
            top_left: title("Farm queue"),
            ..Titles::default()
        };
        boxed(buf, area, theme::border(), &titles, &lines);
        return 0;
    }
    let Ok(c) = queue_cols(inner, q.pip_width as usize) else {
        return offset;
    };
    let xs = r.class == SizeClass::Xs;
    let says = RuleSays {
        value: !xs,
        hint: !xs,
        done_shown: cx.app.show_done,
    };
    let times = Times {
        clock: cx.app.clock_times,
        now: cx.s.now,
        zone: cx.s.zone,
    };
    let Ok(all) = rows(q, &c, times, says) else {
        return offset;
    };
    let room = usize::from(r.queue_rows());
    // The chosen game's row stays in view.
    let mut offset = offset.min(all.len().saturating_sub(room));
    if let Some(at) = all
        .iter()
        .position(|row| matches!(row, Row::Game(id, _) if Some(*id) == cx.app.selected))
    {
        if at < offset {
            offset = at;
        } else if at >= offset + room {
            offset = at + 1 - room;
        }
    }
    let (shown, above, below) = window(&all, room, offset);
    let mut lines = Vec::new();
    if r.queue_header() {
        lines.extend(queue_head(&c).ok());
    }
    lines.extend(shown.iter().map(|row| match row {
        Row::Rule(line) => line.clone(),
        Row::Game(id, line) if Some(*id) == cx.app.selected => selected(line.clone()),
        Row::Game(_, line) => line.clone(),
    }));
    let more = more(above, below);
    let titles = if xs {
        xs_titles(p, &more, w)
    } else {
        let eta = p
            .eta
            .as_ref()
            .filter(|e| r.class == SizeClass::L && !e.assumed && !e.holds_still)
            .map(|e| e.eta);
        let budget = w.saturating_sub(if r.class == SizeClass::L { 16 } else { 18 });
        let right = first_fit(queue_title(p.to_go.games, p.to_go.drops, eta), budget).ok();
        let leg = legend(w.saturating_sub(6 + super::text::width(&more) + 4));
        Titles {
            top_left: title("Farm queue"),
            top_right: right.filter(|l| l.width() > 0),
            bottom_left: Some(leg).filter(|l| l.width() > 0),
            bottom_right: Some(Line::styled(more, theme::dim())).filter(|l| l.width() > 0),
        }
    };
    boxed(buf, area, theme::border(), &titles, &lines);
    offset
}

/// The queue's borders at XS, which carry what Progress's panel can't:
/// what's to go, the columns' names, and the library's drops.
fn xs_titles(p: &Progress, more: &str, w: usize) -> Titles {
    let to_go = if p.to_go.games == 0 {
        "nothing to go".to_owned()
    } else {
        format!("{} to go", p.to_go.games)
    };
    let left = first_fit(
        [
            Line::styled(format!("Farm queue · {to_go}"), theme::heading()),
            Line::styled("Farm queue", theme::heading()),
        ],
        w.saturating_sub(30),
    )
    .ok();
    let columns = first_fit(
        [
            Line::styled("hours · drops · ≈ done in", theme::dim()),
            Line::styled("hours · drops", theme::dim()),
        ],
        w.saturating_sub(26),
    )
    .ok();
    let library = first_fit(
        [
            Line::styled(
                format!(
                    "library {} of {} drops",
                    p.library.received, p.library.total
                ),
                theme::dim(),
            ),
            Line::default(),
        ],
        w.saturating_sub(8 + super::text::width(more) + 4),
    )
    .ok()
    .filter(|_| p.summary.is_none());
    Titles {
        top_left: left,
        top_right: columns,
        bottom_left: library.filter(|l| l.width() > 0),
        bottom_right: Some(Line::styled(more.to_owned(), theme::dim())).filter(|l| l.width() > 0),
    }
}

/// A row as the selection shows it: the whole row in the selection's bar.
fn selected(line: Line<'static>) -> Line<'static> {
    let mut line = line;
    for span in &mut line.spans {
        span.style = theme::selected();
    }
    line.style(theme::selected())
}

/// The Now panel at L: what's being played and how, in the state's colour.
fn now_box(buf: &mut Buffer, cx: &Ctx<'_>, p: &Progress, now: &Now, area: Rect) {
    let Ok((rows, since, every)) = now_panel(p, now, cx.spinner, usize::from(area.width) - 4)
    else {
        return;
    };
    let colour = match p.activity {
        Activity::Farming { .. } | Activity::BuildingHours { .. } => theme::GOOD,
        Activity::Waiting { .. } | Activity::Paused | Activity::Reading | Activity::Checking => {
            theme::BUSY
        }
        Activity::Expired { .. }
        | Activity::Reconnecting { .. }
        | Activity::Unreadable { .. }
        | Activity::TakenOver => theme::BAD,
        _ => theme::DIM,
    };
    let titles = Titles {
        top_left: title("Now"),
        top_right: since.map(|t| Line::styled(t, theme::dim())),
        bottom_left: every.map(|t| Line::styled(t, theme::dim())),
        bottom_right: None,
    };
    boxed(buf, area, theme::fg(colour), &titles, &rows);
}

/// The right-hand column: the chosen game, and this session's cards.
fn right(buf: &mut Buffer, cx: &Ctx<'_>, area: Rect) {
    let r = cx.regions;
    let w = usize::from(area.width);
    let rows = usize::from(area.height);
    let reading = matches!(Activity::of(&cx.s), Activity::Reading);
    let game = cx
        .app
        .selected
        .and_then(|id| ChosenGame::build(&cx.s, id))
        .filter(|_| !reading);
    let Some(g) = game else {
        let h = rows.min(8);
        let lines = wrap(
            "Nothing to show until the badges are read: a game's cards, prices and priority \
             appear here.",
            w.saturating_sub(4),
            0,
        )
        .unwrap_or_default();
        let titles = Titles {
            top_left: title("The chosen game"),
            ..Titles::default()
        };
        boxed(
            buf,
            Rect {
                height: h as u16,
                ..area
            },
            theme::border(),
            &titles,
            &lines,
        );
        haul(buf, cx, area, h as u16);
        return;
    };
    let Ok(forms) = chosen_forms(&g, w.saturating_sub(4)) else {
        return;
    };
    let (form, height, with_haul) = if r.class == SizeClass::L {
        let at = forms
            .iter()
            .position(|f| f.len() + 2 <= 16)
            .unwrap_or(forms.len() - 1);
        (at, 16.min(rows), true)
    } else {
        split_m(&forms, rows)
    };
    let hint = first_fit(
        [
            Line::from(vec![
                key("enter"),
                Span::styled(" everything about it   ", theme::dim()),
                key("o"),
                Span::styled(" card page", theme::dim()),
            ]),
            Line::from(vec![
                key("enter"),
                Span::styled(" all   ", theme::dim()),
                key("o"),
                Span::styled(" card page", theme::dim()),
            ]),
        ],
        w.saturating_sub(6),
    )
    .ok();
    let titles = Titles {
        top_left: Some(Line::styled(g.name.clone(), theme::heading())),
        top_right: Some(Line::styled("selected", theme::dim())),
        bottom_left: hint,
        bottom_right: None,
    };
    let titles = if super::text::titles_fit(w, titles.top_left.as_ref(), titles.top_right.as_ref())
    {
        titles
    } else {
        Titles {
            top_right: None,
            ..titles
        }
    };
    let chosen_area = Rect {
        height: height as u16,
        ..area
    };
    boxed(
        buf,
        chosen_area,
        theme::fg(theme::SELECT),
        &titles,
        &forms[form],
    );
    if with_haul {
        haul(buf, cx, area, height as u16);
    }
}

/// This session's cards, below the chosen game's `top` rows.
fn haul(buf: &mut Buffer, cx: &Ctx<'_>, column: Rect, top: u16) {
    let area = Rect {
        y: column.y + top,
        height: column.height.saturating_sub(top),
        ..column
    };
    if area.height < 3 {
        return;
    }
    let h = Haul::build(&cx.s);
    let w = usize::from(area.width);
    let inner = w.saturating_sub(4);
    let rows = usize::from(area.height) - 2;
    let dim = |t: String| Line::styled(t, theme::dim());
    let at_l = cx.regions.class == SizeClass::L;
    if h.rows.is_empty() {
        let mut lines = wrap(
            "No cards yet. The first usually drops within about half an hour of a game being \
             played on its own, and shows here a few seconds later, with its price.",
            inner,
            0,
        )
        .unwrap_or_default();
        if lines.len() + 5 <= rows {
            lines.push(Line::default());
            lines.extend(
                wrap(
                    Line::from(vec![
                        Span::raw("Prices are what buyers pay now. "),
                        key("b"),
                        Span::raw(" switches to what you'd get after Steam's fees, or by selling at once."),
                    ]),
                    inner,
                    0,
                )
                .unwrap_or_default(),
            );
        }
        let titles = Titles {
            top_left: title("This session · no cards yet"),
            bottom_left: Some(Line::from(vec![
                key("h"),
                Span::styled(" all", theme::dim()),
            ])),
            ..Titles::default()
        };
        boxed(buf, area, theme::border(), &titles, &lines);
        return;
    }
    let held = h.total.as_ref();
    let unpriced = held.map_or(0, |t| t.unpriced);
    let lines = if at_l {
        haul_rows_l(&h, rows, cx.spinner, inner)
    } else {
        haul_rows_m(&h, rows, cx.spinner, inner)
    };
    let Ok((lines, start)) = lines else {
        return;
    };
    let n = h.rows.len();
    let (name, value) = if at_l {
        (
            format!("This session · {}", super::format::cards(n)),
            held.map(super::format::held),
        )
    } else {
        (
            format!("This session · {n}"),
            held.map(super::format::at_least),
        )
    };
    let earlier = |long: bool| {
        let mut spans = vec![Span::styled(format!("{start} earlier ↑ · "), theme::dim())];
        if long {
            spans.push(key("h"));
            spans.push(Span::styled(" all", theme::dim()));
        }
        Line::from(spans)
    };
    let bottom_left = if start > 0 {
        first_fit(
            [earlier(true), dim(format!("{start} more ↑"))],
            w.saturating_sub(18),
        )
        .ok()
    } else if at_l {
        Some(Line::from(vec![
            key("h"),
            Span::styled(" every card, by game", theme::dim()),
        ]))
    } else {
        Some(Line::from(vec![
            key("h"),
            Span::styled(" all", theme::dim()),
        ]))
    };
    let used = bottom_left.as_ref().map_or(0, Line::width);
    let bottom_right = if at_l {
        (h.spares > 0).then(|| {
            let spares = h
                .rows
                .iter()
                .filter(|r| r.copy.is_some_and(|c| c > 1))
                .filter_map(|r| r.price.and_then(|p| p.value()))
                .fold(None, |sum: Option<market::Money>, v| {
                    Some(sum.map_or(v, |s| s.checked_add(v).unwrap_or(s)))
                });
            dim(match spares {
                Some(v) => format!(
                    "{}, {}",
                    super::format::spares(h.spares),
                    super::format::money(v)
                ),
                None => super::format::spares(h.spares),
            })
        })
    } else {
        first_fit(
            [dim(format!("{unpriced} unpriced")), Line::default()],
            w.saturating_sub(6 + used + 4),
        )
        .ok()
        .filter(|_| unpriced > 0)
    };
    let titles = Titles {
        top_left: title(&name),
        top_right: value.map(|v| Line::styled(v, theme::bold())),
        bottom_left,
        bottom_right: bottom_right.filter(|l| l.width() > 0),
    };
    let fits = super::text::titles_fit(w, titles.top_left.as_ref(), titles.top_right.as_ref())
        && super::text::titles_fit(w, titles.bottom_left.as_ref(), titles.bottom_right.as_ref());
    if fits {
        boxed(buf, area, theme::border(), &titles, &lines);
    }
}
