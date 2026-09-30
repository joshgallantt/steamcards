// The main screen: a header, what's being played right now, the farm queue,
// details for the selected game, recent events, and key hints.
//
// Every game has one state — farming (played alone, its cards dropping),
// building hours (played with others), queued, or done — drawn the same way
// everywhere: the queue's STATUS column, the section summaries, the Now
// panel and the details panel.
//
// The selected queue row is a solid bar in the selection colour; the details
// panel wears the same colour and the bar runs into it like a tab.
//
// Text adapts to the space it has: each piece comes in a few lengths and the
// longest one that fits is drawn, so nothing is cut off mid-sentence.

use std::time::Duration;

use chrono::{DateTime, Utc};
use farming::{Mode, Status};
use preferences::Tier;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

use super::{
    Ctx, Logged, WIDE, layout,
    theme::{self, BAD, BUSY, DIM, GOOD, LINK, SELECT},
    widgets::{
        DIVIDER, bar, elapsed, first_fit, fit, fit_right, hints, keycap, panel, rule, selected_row,
        spread, truncate, width, wrap_list, wrap_text,
    },
};
use crate::viewmodel::{LogKind, QueueEntry, Section, Strip};

/// Hours a game needs before its cards drop: what the farmer works to.
const HOURS_BEFORE_DROPS: f64 = 3.0;
/// Width of the "12.5h" column, with its leading space.
const HOURS: usize = 8;
/// Width of the "1/4" column, with its leading space.
const CARDS: usize = 7;
/// Width of the field labels in the details panel.
const LABEL: usize = 10;
/// Log lines older than this fade in the dashboard strip.
const FRESH: Duration = Duration::from_secs(20);

/// Draws the dashboard; returns the queue's scroll offset to remember.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) -> usize {
    let strip = match area.height {
        h if h >= 34 => 3,
        h if h >= 26 => 2,
        _ => 1,
    };
    let [header, body, strip_area, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(6),
        Constraint::Length(strip),
        Constraint::Length(1),
    ])
    .areas(area);

    render_header(f, header, cx);
    let now = now_lines(cx, body.width.saturating_sub(4) as usize);
    let now_h = (now.len() as u16 + 2).min(body.height.saturating_sub(6));
    let [now_area, main] =
        Layout::vertical([Constraint::Length(now_h), Constraint::Min(5)]).areas(body);
    let state = now_color(cx);
    let now_block =
        panel(Line::styled(" Now ", theme::strong(state))).border_style(theme::fg(state));
    f.render_widget(Paragraph::new(now).block(now_block), now_area);
    let offset = if area.width >= WIDE {
        let [q, d] = Layout::horizontal([Constraint::Percentage(57), Constraint::Percentage(43)])
            .areas(main);
        let (offset, selected_y) = render_queue(f, q, cx);
        render_detail(f, d, cx);
        if let Some(y) = selected_y.filter(|_| cx.selected().is_some()) {
            connect(f, q, d, y);
        }
        offset
    } else {
        render_queue(f, main, cx).0
    };
    render_strip(f, strip_area, cx);
    render_footer(f, footer, cx);
    offset
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

/// Joins the selected row to the details panel like a tab: the selection bar
/// runs through the queue's padding and right edge and meets a junction on the
/// details panel's border.
fn connect(f: &mut Frame<'_>, queue: Rect, detail: Rect, y: u16) {
    let buf = f.buffer_mut();
    for x in [queue.right() - 2, queue.right() - 1] {
        buf[(x, y)].set_symbol(" ").set_style(theme::selected());
    }
    buf[(detail.x, y)]
        .set_symbol("┤")
        .set_style(theme::fg(SELECT));
}

// ── Game state ───────────────────────────────────────────────────────────────

/// What a game is doing, drawn the same way everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum State {
    /// Played on its own: its cards are dropping.
    Farming,
    /// Played with others, building hours.
    Hours,
    Queued,
    Done,
}

impl State {
    pub(super) fn of(e: &QueueEntry) -> Self {
        if !e.game.has_drops_left() {
            Self::Done
        } else {
            match e.playing {
                Some(Mode::Cards) => Self::Farming,
                Some(Mode::Hours) => Self::Hours,
                None => Self::Queued,
            }
        }
    }

    fn symbol(self) -> &'static str {
        match self {
            Self::Farming => theme::FARMING,
            Self::Hours => theme::HOURS,
            Self::Queued => " ",
            Self::Done => theme::DONE,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Farming => "farming",
            Self::Hours => "hours",
            Self::Queued => "",
            Self::Done => "done",
        }
    }

    fn style(self) -> Style {
        match self {
            Self::Farming => theme::strong(GOOD),
            Self::Hours | Self::Done => theme::fg(GOOD),
            Self::Queued => theme::dim(),
        }
    }
}

/// "5.2h", "12h", "1,234h".
pub(super) fn hours(h: f64) -> String {
    if h < 10.0 {
        format!("{h:.1}h")
    } else {
        let whole = h.round() as u64;
        let digits = whole.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i).is_multiple_of(3) {
                out.push(',');
            }
            out.push(c);
        }
        format!("{out}h")
    }
}

/// "in 12m", "in 1h 5m", "now".
fn countdown(at: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (at - now).num_seconds();
    if secs <= 30 {
        "now".to_owned()
    } else {
        format!("in {}", elapsed(Duration::from_secs(secs as u64 + 30)))
    }
}

/// "1 card", "3 cards".
fn cards(n: u32) -> String {
    if n == 1 {
        "1 card".to_owned()
    } else {
        format!("{n} cards")
    }
}

// ── Header ───────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let app = cx.app;
    let w = area.width as usize;
    let dropped = app.dropped();

    // Left: the app, whether it's farming, and what's dropped — in
    // decreasing detail.
    let left = |detail: usize| {
        let mut v = vec![Span::styled(" steamcards ", theme::pill()), Span::raw("  ")];
        if app.farming.is_running() {
            v.push(Span::styled(
                format!("{} farming", theme::SIGNED_IN),
                theme::fg(GOOD),
            ));
            if let (Some(d), true) = (app.farming.running_for(), detail >= 2) {
                v.push(dim(format!(" for {}", elapsed(d))));
            }
        } else if app.paused_by_user {
            v.push(Span::styled(
                format!("{} paused", theme::PAUSED),
                theme::fg(BUSY),
            ));
        } else {
            v.push(dim(format!("{} not farming", theme::IDLE)));
        }
        if dropped > 0 && detail >= 1 {
            let what = if detail >= 2 { " this session" } else { "" };
            v.push(Span::styled(
                format!("   {} {}{what}", theme::DONE, cards(dropped)),
                theme::fg(GOOD),
            ));
        }
        v
    };
    // Right: the account, how it shows to friends, then help.
    let right = |detail: usize| {
        let mut v = vec![Span::styled("Steam", theme::steam()), Span::raw(" ")];
        if detail >= 2 {
            v.extend(account_badge(cx));
        } else {
            v.push(match cx.account {
                Some(a) if a.expired => Span::styled(theme::FAILED, theme::fg(BAD)),
                Some(_) => Span::styled(theme::SIGNED_IN, theme::fg(GOOD)),
                None => dim(theme::IDLE),
            });
        }
        if cx.signed_in() && detail >= 1 {
            v.push(dim(if cx.prefs.appear_online {
                " · online"
            } else {
                " · appears offline"
            }));
        }
        v.push(Span::raw("   "));
        v.push(keycap("?"));
        v.push(dim(" help "));
        v
    };
    let pairs = [(2, 2), (1, 2), (2, 1), (1, 1), (0, 1), (0, 0)];
    let (l, r) = pairs
        .iter()
        .map(|&(a, b)| (left(a), right(b)))
        .find(|(l, r)| width(l) + 1 + width(r) <= w)
        .unwrap_or_else(|| (left(0), right(0)));
    f.render_widget(Paragraph::new(spread(l, r, w)), area);
}

/// "● cardfarmer", "✕ sign-in expired" or "○ not signed in".
pub(super) fn account_badge(cx: &Ctx<'_>) -> Vec<Span<'static>> {
    match cx.account {
        Some(a) if a.expired => vec![Span::styled(
            format!("{} sign-in expired", theme::FAILED),
            theme::fg(BAD),
        )],
        Some(a) => {
            let who = if a.name.is_empty() {
                "signed in"
            } else {
                a.name.as_str()
            };
            vec![Span::styled(
                format!("{} {who}", theme::SIGNED_IN),
                theme::fg(GOOD),
            )]
        }
        None => vec![dim(format!("{} not signed in", theme::IDLE))],
    }
}

// ── Now ──────────────────────────────────────────────────────────────────────

/// The Now panel's border mirrors the overall state at a glance.
fn now_color(cx: &Ctx<'_>) -> Color {
    let status = cx
        .app
        .status
        .as_ref()
        .filter(|_| cx.app.farming.is_running());
    match status.map(|s| s.status) {
        _ if cx.expired() => BAD,
        Some(Status::Farming) => GOOD,
        Some(Status::Error) => BAD,
        Some(Status::Blocked | Status::Checking) => BUSY,
        _ if cx.app.farming.is_running() => BUSY,
        _ => DIM,
    }
}

/// What's happening, in a line or two.
fn now_lines(cx: &Ctx<'_>, w: usize) -> Vec<Line<'static>> {
    // A status line whose text is cut short, if need be, before its hint.
    let line = |glyph: Span<'static>, text: String, style: Style, hint: Vec<Span<'static>>| {
        let hint = if 2 + text.chars().count() + width(&hint) <= w {
            hint
        } else {
            Vec::new()
        };
        let room = w.saturating_sub(2 + width(&hint));
        let mut spans = vec![glyph, Span::styled(truncate(&text, room), style)];
        spans.extend(hint);
        Line::from(spans)
    };
    let glyph = |g: &str, style| Span::styled(format!("{g} "), style);
    let press = |k: &str, what: &str| vec![dim("  ·  press "), keycap(k), dim(format!(" {what}"))];

    if !cx.signed_in() {
        return vec![line(
            glyph(theme::IDLE, theme::dim()),
            "Not signed in".into(),
            theme::dim(),
            press("a", "to sign in"),
        )];
    }
    if cx.expired() {
        return vec![line(
            glyph(theme::FAILED, theme::fg(BAD)),
            "Steam no longer takes the saved sign-in".into(),
            theme::fg(BAD),
            press("a", "to sign in again"),
        )];
    }
    if !cx.app.farming.is_running() {
        return vec![line(
            glyph(theme::PAUSED, theme::fg(BUSY)),
            "Paused".into(),
            theme::fg(BUSY),
            press("p", "to carry on"),
        )];
    }
    let busy = glyph(cx.spinner(), theme::fg(BUSY));
    let Some(s) = &cx.app.status else {
        return vec![line(busy, "Starting…".into(), theme::plain(), Vec::new())];
    };
    let next = |what: &str| {
        s.next_look
            .map(|at| vec![dim(format!("  ·  {what} {}", countdown(at, cx.now)))])
            .unwrap_or_default()
    };
    match s.status {
        Status::Farming => match s.mode {
            Some(Mode::Hours) => building_lines(cx, s, w),
            _ => farming_lines(cx, s, w),
        },
        Status::Checking => vec![line(
            busy,
            capitalize(&note_or(&s.note, "reading your badges…")),
            theme::plain(),
            Vec::new(),
        )],
        Status::Idle => vec![line(
            glyph(theme::IDLE, theme::dim()),
            capitalize(&note_or(&s.note, "nothing to farm")),
            theme::plain(),
            next("looks again"),
        )],
        Status::Blocked => {
            let what = s
                .blocked_by
                .and_then(|id| s.library.game(id))
                .map_or_else(String::new, |g| format!(": {}", g.name));
            vec![line(
                glyph(theme::PAUSED, theme::fg(BUSY)),
                format!("Playing on another device{what}"),
                theme::fg(BUSY),
                vec![dim("  ·  farming waits until it stops")],
            )]
        }
        Status::Error => vec![line(
            glyph(theme::FAILED, theme::fg(BAD)),
            capitalize(&note_or(&s.note, "something went wrong")),
            theme::fg(BAD),
            Vec::new(),
        )],
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map_or_else(String::new, |first| first.to_uppercase().chain(c).collect())
}

fn note_or(note: &str, fallback: &str) -> String {
    if note.is_empty() {
        fallback.to_owned()
    } else {
        note.to_owned()
    }
}

/// One game on its own: what, when it's next looked at, then its drops.
fn farming_lines(cx: &Ctx<'_>, s: &farming::FarmingStatus, w: usize) -> Vec<Line<'static>> {
    let Some(game) = s.playing.first().and_then(|&id| s.library.game(id)) else {
        return Vec::new();
    };
    let tier = match cx.prefs.tier(game.app_id) {
        Tier::Priority(n) => vec![Span::styled(format!("  #{n}"), theme::strong(BUSY))],
        _ => Vec::new(),
    };
    let left = |verbose: bool| {
        let mut v = vec![Span::styled(
            format!(
                "{} {}",
                theme::FARMING,
                if verbose { "Farming " } else { "" }
            ),
            theme::fg(GOOD),
        )];
        v.push(Span::styled(game.name.clone(), theme::strong(GOOD)));
        v.extend(tier.clone());
        v
    };
    let look = s.next_look.map(|at| countdown(at, cx.now));
    let right = |verbose: bool| match &look {
        Some(t) if verbose => vec![dim("next look "), Span::styled(t.clone(), theme::bold())],
        Some(t) => vec![Span::styled(t.clone(), theme::bold())],
        None => Vec::new(),
    };
    let options = [
        (left(true), right(true)),
        (left(false), right(true)),
        (left(false), right(false)),
    ];
    let (l, r) = options
        .iter()
        .find(|(l, r)| width(l) + 1 + width(r) <= w)
        .unwrap_or(&options[2])
        .clone();
    let mut lines = vec![spread(l, r, w)];

    // Its drops, as a bar, with the numbers beside it.
    let nums = format!(
        "  {}/{} cards · {} played",
        game.drops.received,
        game.drops.total(),
        hours(game.hours)
    );
    let bw = w.saturating_sub(2 + nums.chars().count());
    let mut l = vec![Span::raw("  ")];
    l.extend(bar(
        game.drops.received as i32,
        game.drops.total() as i32,
        bw,
        theme::fg(GOOD),
    ));
    l.push(Span::raw(nums));
    lines.push(Line::from(l));
    lines
}

/// Several games together, building hours: how many, and how close the
/// leading one is to its cards dropping.
fn building_lines(cx: &Ctx<'_>, s: &farming::FarmingStatus, w: usize) -> Vec<Line<'static>> {
    let lead = s
        .playing
        .iter()
        .filter_map(|&id| s.library.game(id))
        .max_by(|a, b| a.hours.total_cmp(&b.hours));
    let Some(lead) = lead else {
        return Vec::new();
    };
    let n = s.playing.len();
    let what = if n == 1 {
        lead.name.clone()
    } else {
        format!("{n} games")
    };
    let left = |verbose: bool| {
        let mut v = vec![Span::styled(
            format!("{} Building hours on ", theme::HOURS),
            theme::fg(GOOD),
        )];
        v.push(Span::styled(what.clone(), theme::strong(GOOD)));
        if verbose {
            v.push(dim("  ·  cards drop from 3 hours"));
        }
        v
    };
    let ready = s.next_look.map(|at| countdown(at, cx.now));
    let right = |verbose: bool| match &ready {
        Some(t) if verbose && n > 1 => vec![
            Span::raw(lead.name.clone()),
            dim(" ready "),
            Span::styled(t.clone(), theme::bold()),
        ],
        Some(t) => vec![dim("ready "), Span::styled(t.clone(), theme::bold())],
        None => Vec::new(),
    };
    let options = [
        (left(true), right(true)),
        (left(false), right(true)),
        (left(false), right(false)),
    ];
    let (l, r) = options
        .iter()
        .find(|(l, r)| width(l) + 1 + width(r) <= w)
        .unwrap_or(&options[2])
        .clone();
    let mut lines = vec![spread(l, r, w)];

    let nums = format!("  {} of 3h · {}", hours(lead.hours), lead.name);
    let nums = truncate(&nums, w.saturating_sub(12));
    let bw = w.saturating_sub(2 + nums.chars().count());
    let mut l = vec![Span::raw("  ")];
    l.extend(bar(
        (lead.hours * 60.0) as i32,
        (HOURS_BEFORE_DROPS * 60.0) as i32,
        bw,
        theme::fg(LINK),
    ));
    l.push(Span::raw(nums));
    lines.push(Line::from(l));
    lines
}

// ── Queue ────────────────────────────────────────────────────────────────────

/// Width of the STATUS column as a symbol ("▶ ") and spelled out
/// ("▶ farming ").
const STATUS_SYMBOL: usize = 2;
const STATUS_WORDS: usize = 10;

/// Column widths for queue rows, fitted to the panel: the game's name keeps a
/// sensible minimum, and the spelled-out status and progress bar join (in
/// that order) when there's room for them.
struct Cols {
    words: bool,
    name: usize,
    bar: usize,
}

impl Cols {
    fn new(w: usize) -> Self {
        const NAME_MIN: usize = 14;
        let mut avail = w.saturating_sub(1 + STATUS_SYMBOL + 4 + HOURS + CARDS);
        let mut take = |n: usize| {
            let ok = avail >= NAME_MIN + n;
            if ok {
                avail -= n;
            }
            ok
        };
        let words = take(STATUS_WORDS - STATUS_SYMBOL);
        let mut bar = if take(9) { 8 } else { 0 };
        // Spare room widens the bar a little; the rest is the name's.
        if bar > 0 {
            let extra = avail.saturating_sub(NAME_MIN + 16).min(6);
            bar += extra;
            avail -= extra;
        }
        Self {
            words,
            name: avail,
            bar,
        }
    }

    fn status(&self) -> usize {
        if self.words {
            STATUS_WORDS
        } else {
            STATUS_SYMBOL
        }
    }
}

/// Draws the queue; returns its scroll offset and the selected row's screen
/// row (when visible) so the details panel can be joined to it.
fn render_queue(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) -> (usize, Option<u16>) {
    let q = cx.queue;
    let mut block = panel(Line::styled(" Farm queue ", theme::heading()));
    let inner = block.inner(area);

    if q.is_empty() {
        f.render_widget(block, area);
        render_queue_empty(f, inner, cx);
        return (0, None);
    }

    let to_go = q
        .entries()
        .filter(|e| e.wanted && matches!(e.section(), Section::Priority | Section::Indifferent))
        .count();
    let drops = q.drops_to_go();
    block = block
        .title_top(Line::from(dim(format!(" {to_go} to go · {} ", cards(drops)))).right_aligned());

    let w = inner.width as usize;
    let cols = Cols::new(w);
    let [head_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let h = list_area.height as usize;

    // An empty PRIORITY section still shows, with instructions — it's the
    // feature people most need to discover.
    let sections: Vec<(Section, &Vec<QueueEntry>)> = q
        .sections
        .iter()
        .filter(|r| !r.entries.is_empty())
        .map(|r| (r.section, &r.entries))
        .collect();
    let no_priority = !sections.iter().any(|(s, _)| *s == Section::Priority) && to_go > 0;
    let show_done = cx.app.show_done;
    let rows_shown = |s: Section, n: usize| {
        if s == Section::Done && !show_done {
            0
        } else {
            n
        }
    };
    let groups = sections.len() + usize::from(no_priority);
    let total: usize = sections
        .iter()
        .map(|(s, v)| 1 + rows_shown(*s, v.len()))
        .sum::<usize>()
        + if no_priority { 2 } else { 0 };
    // Breathing room between sections, when it all fits anyway.
    let roomy = total + groups.saturating_sub(1) <= h;

    let mut lines = Vec::new();
    let mut is_heading = Vec::new();
    let mut sel = None;
    if no_priority {
        lines.push(section_rule(Section::Priority, &[], cx, w));
        let hint = first_fit(
            vec![
                vec![
                    dim("   Nothing ranked yet — select a game and press "),
                    keycap("1"),
                    dim(" to farm it first."),
                ],
                vec![
                    dim("   Nothing ranked yet — press "),
                    keycap("1"),
                    dim(" on a game."),
                ],
                vec![dim("   Press "), keycap("1"), dim(" to rank a game.")],
            ],
            w,
        )
        .unwrap_or_default();
        lines.push(Line::from(hint));
        is_heading.extend([true, true]);
    }
    for (i, &(section, entries)) in sections.iter().enumerate() {
        if roomy && (i > 0 || no_priority) {
            lines.push(Line::default());
            is_heading.push(true);
        }
        lines.push(section_rule(section, entries, cx, w));
        is_heading.push(true);
        if rows_shown(section, entries.len()) == 0 {
            continue;
        }
        for e in entries {
            let row = queue_row(e, &cols);
            if cx.app.selected == Some(e.game.app_id) {
                sel = Some(lines.len());
                lines.push(selected_row(row, w));
            } else {
                lines.push(row);
            }
            is_heading.push(false);
        }
    }

    // Scroll just enough to keep the cursor (and its section heading) in view.
    let max = lines.len().saturating_sub(h);
    let mut off = cx.app.queue_offset.min(max);
    if let Some(s) = sel {
        let mut top = s;
        while top > 0 && is_heading[top - 1] {
            top -= 1;
        }
        if top < off {
            off = top;
        }
        if s >= off + h {
            off = s + 1 - h;
        }
    }
    if max > 0 {
        let more = match (off > 0, off < max) {
            (true, true) => " ↑↓ more ",
            (true, false) => " ↑ more ",
            _ => " ↓ more ",
        };
        block = block.title_bottom(Line::from(dim(more)).right_aligned());
    }
    // What the symbols mean, written into the bottom edge.
    let room = (area.width as usize).saturating_sub(if max > 0 { 16 } else { 4 });
    if let Some(legend) = legend(room, cols.words) {
        block = block.title_bottom(legend);
    }
    f.render_widget(block, area);
    f.render_widget(Paragraph::new(column_header(&cols)), head_area);
    f.render_widget(Paragraph::new(lines).scroll((off as u16, 0)), list_area);
    if max > 0 {
        let bar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("│"))
            .thumb_symbol("┃")
            .track_style(theme::border())
            .thumb_style(theme::plain());
        let mut state = ScrollbarState::new(max + 1)
            .viewport_content_length(h)
            .position(off);
        f.render_stateful_widget(
            bar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut state,
        );
    }
    let selected_y = sel
        .filter(|&s| s >= off && s < off + h)
        .map(|s| list_area.y + (s - off) as u16);
    (off, selected_y)
}

/// The symbols that aren't already spelled out on screen, as many as fit.
fn legend(room: usize, status_in_words: bool) -> Option<Line<'static>> {
    let mut items = Vec::new();
    if !status_in_words {
        items.extend([
            (Span::styled(theme::FARMING, theme::fg(GOOD)), "farming"),
            (
                Span::styled(theme::HOURS, theme::fg(GOOD)),
                "building hours",
            ),
        ]);
    }
    items.extend([
        (Span::styled("#1", theme::strong(BUSY)), "priority"),
        (Span::styled(theme::SKIPPED, theme::fg(BAD)), "skipped"),
    ]);
    if !status_in_words {
        items.push((Span::styled(theme::DONE, theme::fg(GOOD)), "done"));
    }
    let mut spans = vec![Span::raw(" ")];
    let mut used = 1;
    let mut shown = 0;
    for (glyph, what) in items {
        let cost = glyph.content.chars().count() + 1 + what.len() + 2;
        if used + cost > room {
            break;
        }
        used += cost;
        shown += 1;
        spans.push(glyph);
        spans.push(dim(format!(" {what}  ")));
    }
    (shown >= 2).then(|| Line::from(spans))
}

fn column_header(c: &Cols) -> Line<'static> {
    let mut s = String::from(" ");
    s.push_str(&fit(if c.words { "STATUS" } else { "" }, c.status()));
    s.push_str(&fit("#", 4));
    s.push_str(&fit("GAME", c.name));
    s.push_str(&fit_right("HOURS", HOURS));
    if c.bar > 0 {
        s.push(' ');
        s.push_str(&fit("DROPS", c.bar));
    }
    s.push_str(&fit_right("CARDS", CARDS));
    Line::styled(s, theme::dim())
}

/// A section's divider: its name and size, what of it is being played (when
/// that says something), and what the tier means — as much as fits.
fn section_rule(section: Section, entries: &[QueueEntry], cx: &Ctx<'_>, w: usize) -> Line<'static> {
    let only = cx.prefs.only_priority;
    let (name, style, hints) = match section {
        Section::Priority => (
            "PRIORITY",
            theme::strong(BUSY),
            vec![
                vec![dim("farmed first, in your order")],
                vec![dim("farmed first")],
            ],
        ),
        Section::Indifferent if only => (
            "INDIFFERENT",
            theme::bold(),
            vec![
                vec![dim("not farmed: \"only priority\" is on")],
                vec![dim("not farmed")],
            ],
        ),
        Section::Indifferent => (
            "INDIFFERENT",
            theme::bold(),
            vec![
                vec![dim("after your priorities · closest to dropping first")],
                vec![dim("closest to dropping first")],
                vec![dim("no preference")],
            ],
        ),
        Section::Skipped => (
            "SKIPPED",
            theme::strong(BAD),
            vec![vec![dim("never farmed")]],
        ),
        Section::Done => (
            "DONE",
            theme::strong(GOOD),
            vec![vec![
                keycap("c"),
                dim(if cx.app.show_done {
                    " to hide"
                } else {
                    " to show"
                }),
            ]],
        ),
    };
    let mut title = vec![
        Span::styled(name, style),
        dim(format!(" · {}", entries.len())),
    ];
    let farming = entries
        .iter()
        .filter(|e| State::of(e) == State::Farming)
        .count();
    let building = entries
        .iter()
        .filter(|e| State::of(e) == State::Hours)
        .count();
    if farming > 0 {
        title.push(Span::styled(" · farming", theme::fg(GOOD)));
    } else if building > 0 {
        title.push(Span::styled(" · building hours", theme::fg(GOOD)));
    }
    rule(title, hints, w, style)
}

fn queue_row(e: &QueueEntry, c: &Cols) -> Line<'static> {
    let state = State::of(e);
    let done = state == State::Done;
    let muted = done || e.tier == Tier::Skip || !e.wanted;

    let status = if c.words {
        format!("{} {}", state.symbol(), state.word())
    } else {
        state.symbol().to_owned()
    };
    let mut s = vec![
        Span::raw(" "),
        Span::styled(fit(&status, c.status()), state.style()),
    ];
    s.push(match e.tier {
        Tier::Priority(n) if !done => Span::styled(fit(&format!("#{n}"), 4), theme::strong(BUSY)),
        Tier::Skip if !done => Span::styled(fit(theme::SKIPPED, 4), theme::fg(BAD)),
        _ => Span::raw("    "),
    });
    s.push(Span::styled(
        fit(&e.game.name, c.name),
        if muted { theme::dim() } else { theme::plain() },
    ));
    // Hours: short of 3, its cards can't drop yet.
    let hours_style = if muted {
        theme::dim()
    } else if e.game.hours < HOURS_BEFORE_DROPS {
        theme::fg(BUSY)
    } else {
        theme::plain()
    };
    s.push(Span::styled(
        fit_right(&hours(e.game.hours), HOURS),
        hours_style,
    ));
    let (got, all) = (e.game.drops.received, e.game.drops.total());
    if c.bar > 0 {
        let fill = match state {
            State::Farming | State::Done => theme::fg(GOOD),
            _ if muted => theme::dim(),
            _ => theme::fg(LINK),
        };
        s.push(Span::raw(" "));
        s.extend(bar(got as i32, all as i32, c.bar, fill));
    }
    s.push(Span::styled(
        fit_right(&format!("{got}/{all}"), CARDS),
        if muted { theme::dim() } else { theme::plain() },
    ));
    Line::from(s)
}

fn render_queue_empty(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let app = cx.app;
    let reading = app.farming.is_running()
        && app
            .status
            .as_ref()
            .is_none_or(|s| s.status == Status::Checking);
    let lines = if reading {
        vec![
            Line::from(vec![
                Span::styled(format!("{} ", cx.spinner()), theme::fg(BUSY)),
                Span::raw("Reading your badges"),
            ]),
            Line::styled("The first read can take a minute.", theme::dim()),
        ]
    } else if !app.farming.is_running() {
        vec![
            Line::styled(
                format!("{} Farming is paused", theme::PAUSED),
                theme::fg(BUSY),
            ),
            Line::from(vec![dim("press "), keycap("p"), dim(" to start")]),
        ]
    } else {
        vec![
            Line::raw("No games with trading cards"),
            Line::styled("Games you own with cards show up here.", theme::dim()),
        ]
    };
    let r = Rect {
        y: area.y + (area.height / 2).saturating_sub(1),
        height: area.height.min(lines.len() as u16),
        ..area
    };
    f.render_widget(Paragraph::new(lines).centered(), r);
}

// ── Details ──────────────────────────────────────────────────────────────────

fn render_detail(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::fg(SELECT))
        .padding(Padding::horizontal(1));
    let Some(e) = cx.selected() else {
        let block = block.title(Line::styled(" Details ", theme::strong(SELECT)));
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new(Line::styled(
                "Choose a game on the left to see it here.",
                theme::dim(),
            )),
            inner,
        );
        return;
    };
    let w = area.width.saturating_sub(4) as usize;
    let name = truncate(&e.game.name, w.saturating_sub(12));
    let block = block
        .title(Line::styled(format!(" {name} "), theme::strong(SELECT)))
        .title_top(Line::from(dim(" selected ")).right_aligned());
    let inner = block.inner(area);
    f.render_widget(block, area);

    // The priority controls follow the details; if space runs short they stay
    // pinned to the bottom and the details give way above them.
    let w = inner.width as usize;
    let mut info = detail_info(e, cx, w);
    let controls = tier_controls(e, w);
    if info.len() + 1 + controls.len() <= inner.height as usize {
        info.push(Line::default());
        info.extend(controls);
        f.render_widget(Paragraph::new(info), inner);
    } else {
        let [top, bottom] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(controls.len() as u16),
        ])
        .areas(inner);
        f.render_widget(Paragraph::new(info), top);
        f.render_widget(Paragraph::new(controls), bottom);
    }
}

/// Everything about one game — its state and what that means, hours, drops,
/// its card set — for the side panel or the pop-up.
pub(super) fn detail_info(e: &QueueEntry, cx: &Ctx<'_>, w: usize) -> Vec<Line<'static>> {
    let g = &e.game;
    let state = State::of(e);
    let mut out = Vec::new();

    let badge = match g.badge_level {
        0 => "no badge yet".to_owned(),
        n => format!("badge level {n}"),
    };
    out.push(Line::from(vec![
        dim(format!("App {}", g.app_id)),
        dim("  ·  "),
        dim(badge),
    ]));
    out.push(Line::default());

    // The same state the queue shows, then what it means for this game.
    let others = cx
        .queue
        .entries()
        .filter(|x| x.playing.is_some() && x.game.app_id != g.app_id)
        .count();
    let first = cx
        .app
        .status
        .as_ref()
        .and_then(|s| s.order.first().copied());
    let (headline, meaning): (Span<'static>, String) = match state {
        State::Farming => (
            Span::styled(format!("{} Farming now", theme::FARMING), state.style()),
            "Played on its own, so its cards can drop. Its card page is looked at every quarter \
             of an hour, and as soon as Steam says new items arrived."
                .to_owned(),
        ),
        State::Hours => (
            Span::styled(
                if others == 0 {
                    format!("{} Building hours", theme::HOURS)
                } else {
                    format!("{} Building hours, with {others} others", theme::HOURS)
                },
                state.style(),
            ),
            format!(
                "Its cards can drop once it has 3 hours on record: {} to go.",
                hours((HOURS_BEFORE_DROPS - g.hours).max(0.0))
            ),
        ),
        State::Done => (
            Span::styled(
                format!("{} Every card has dropped", theme::DONE),
                state.style(),
            ),
            String::new(),
        ),
        State::Queued if e.tier == Tier::Skip => (
            Span::styled(format!("{} Skipped", theme::SKIPPED), theme::fg(BAD)),
            "Never farmed. Press 0 to farm it after your priorities, or 1-9 to rank it.".to_owned(),
        ),
        State::Queued if !e.wanted => (
            Span::styled("Not farmed", theme::bold()),
            "\"Only priority\" is on, and it isn't one of your priority games.".to_owned(),
        ),
        State::Queued if first == Some(g.app_id) => (
            Span::styled("Next up", theme::bold()),
            "Farmed as soon as what's playing now is done.".to_owned(),
        ),
        State::Queued => (
            Span::styled("Queued", theme::bold()),
            match e.tier {
                Tier::Priority(_) => "Farmed before everything ranked below it.".to_owned(),
                _ => "Farmed after your priority games.".to_owned(),
            },
        ),
    };
    out.push(Line::from(headline));
    out.extend(
        wrap_text(&meaning, w)
            .into_iter()
            .filter(|l| !l.is_empty())
            .map(|l| Line::styled(l, theme::dim())),
    );
    out.push(Line::default());

    let mut played = vec![Span::raw(format!("{} on record", hours(g.hours)))];
    if g.hours < HOURS_BEFORE_DROPS && g.has_drops_left() && w >= LABEL + 30 {
        played.push(dim("  ·  cards drop from 3h"));
    }
    out.push(field("Hours", played));
    out.push(field(
        "Drops",
        if g.has_drops_left() {
            vec![
                Span::raw(format!("{} of {}", g.drops.received, g.drops.total())),
                dim(format!("  ·  {} to go", g.drops.remaining)),
            ]
        } else {
            vec![Span::styled(
                format!("all {} dropped", g.drops.total()),
                theme::fg(GOOD),
            )]
        },
    ));
    let collected = if g.cards.is_empty() {
        vec![dim("not looked at yet")]
    } else {
        vec![Span::raw(format!(
            "{} of {} collected",
            g.cards_collected(),
            g.cards.len()
        ))]
    };
    out.push(field("Cards", collected));
    let mut page = vec![dim(fit("Page", LABEL))];
    page.push(keycap("o"));
    page.push(dim(" open its card page"));
    out.push(Line::from(page));

    if !g.cards.is_empty() {
        out.push(Line::default());
        out.push(rule(
            vec![Span::styled("Cards", theme::heading())],
            vec![vec![dim(if g.has_full_set() {
                "a full set".to_owned()
            } else {
                format!("{} of {}", g.cards_collected(), g.cards.len())
            })]],
            w,
            theme::dim(),
        ));
        let names: Vec<String> = g
            .cards
            .iter()
            .map(|c| match c.owned {
                0 => c.name.clone(),
                1 => format!("{} {}", theme::DONE, c.name),
                n => format!("{} {} ×{n}", theme::DONE, c.name),
            })
            .collect();
        out.extend(
            wrap_list(&names, w, 3)
                .into_iter()
                .map(|l| Line::styled(l, theme::plain())),
        );
    }
    out
}

/// The game's tier as a set of radio buttons, each with the key that picks
/// it — so ranking is discoverable right where the game is shown.
pub(super) fn tier_controls(e: &QueueEntry, w: usize) -> Vec<Line<'static>> {
    let mut out = vec![rule(
        vec![Span::styled("Farm priority", theme::heading())],
        Vec::new(),
        w,
        theme::dim(),
    )];
    if !e.game.has_drops_left() {
        let done = format!(
            " {} Nothing left to farm: every card has dropped.",
            theme::DONE
        );
        out.extend(
            wrap_text(&done, w)
                .into_iter()
                .map(|l| Line::styled(l, theme::fg(GOOD))),
        );
        return out;
    }
    let priority = match e.tier {
        Tier::Priority(n) => format!("Priority #{n}"),
        _ => "Priority".to_owned(),
    };
    let options = [
        (
            matches!(e.tier, Tier::Priority(_)),
            priority,
            "1-9",
            theme::strong(BUSY),
        ),
        (
            e.tier == Tier::Indifferent,
            "Indifferent".to_owned(),
            "0",
            theme::bold(),
        ),
        (
            e.tier == Tier::Skip,
            "Skip".to_owned(),
            "x",
            theme::strong(BAD),
        ),
    ];
    const OPTION: usize = 13;
    // The descriptions come as a set — long, short, or none — so the three
    // lines always match.
    let room = w.saturating_sub(3 + OPTION + 1 + 5 + 2);
    let sets = [
        [
            "farmed first, in rank order",
            "after your priorities",
            "never farmed",
        ],
        ["farmed first", "after priorities", "never farmed"],
    ];
    let whats = sets
        .iter()
        .find(|set| set.iter().all(|d| d.len() <= room))
        .copied()
        .unwrap_or(["", "", ""]);
    for ((on, label, key, style), what) in options.into_iter().zip(whats) {
        let (radio, label_style) = if on {
            (Span::styled(format!(" {} ", theme::RADIO_ON), style), style)
        } else {
            (dim(format!(" {} ", theme::RADIO_OFF)), theme::plain())
        };
        out.push(Line::from(vec![
            radio,
            Span::styled(fit(&label, OPTION), label_style),
            Span::raw(" "),
            keycap(&format!("{key:^3}")),
            Span::raw("  "),
            dim(what),
        ]));
    }
    out
}

fn field(label: &str, value: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![dim(fit(label, LABEL))];
    spans.extend(value);
    Line::from(spans)
}

// ── Strip and footer ─────────────────────────────────────────────────────────

/// Latest events (not routine looks), newest at the bottom, with any just-now
/// confirmation last. (While a pop-up is open, it draws the confirmation
/// itself, where the pop-up can't cover it.)
fn render_strip(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let h = area.height as usize;
    if h == 0 {
        return;
    }
    // The bottom row is the strip: a flash, a new event, an alert while it
    // lasts, or the newest event. Rows above it, when there are any, show
    // the events before.
    let newest = cx
        .app
        .log
        .iter()
        .rposition(|e| e.entry.kind != LogKind::Progress);
    let mut lines: Vec<Line<'_>> = cx.app.log[..newest.unwrap_or(0)]
        .iter()
        .rev()
        .filter(|e| e.entry.kind != LogKind::Progress)
        .take(h - 1)
        .map(|e| log_line(e, cx.app.clock.zone(), true))
        .collect();
    lines.reverse();
    let strip = if cx.app.overlay.is_some() && matches!(cx.strip, Strip::Flash { .. }) {
        Line::default()
    } else {
        layout::strip::strip(&cx.strip, cx.progress, area.width as usize).unwrap_or_default()
    };
    lines.push(strip);
    let mut out = vec![Line::default(); h.saturating_sub(lines.len())];
    out.extend(lines);
    f.render_widget(Paragraph::new(out), area);
}

/// A log entry: time, icon, text. `fade` greys out older entries.
pub(super) fn log_line(e: &Logged, zone: chrono::FixedOffset, fade: bool) -> Line<'static> {
    let kind = e.entry.kind;
    let style = match kind {
        LogKind::Dropped | LogKind::Playing => theme::fg(GOOD),
        LogKind::MovedOn => theme::fg(LINK),
        LogKind::Waiting | LogKind::Warning => theme::fg(BUSY),
        LogKind::Error => theme::fg(BAD),
        LogKind::Info | LogKind::Progress => theme::dim(),
    };
    let stale = fade && e.received.elapsed() > FRESH;
    let text_style = match kind {
        _ if stale => theme::dim(),
        LogKind::Progress => theme::dim(),
        LogKind::Dropped => theme::fg(GOOD),
        LogKind::Error => theme::fg(BAD),
        _ => theme::plain(),
    };
    let at = e.entry.at.with_timezone(&zone);
    let time = if fade {
        at.format("%H:%M")
    } else {
        at.format("%H:%M:%S")
    };
    Line::from(vec![
        dim(format!(" {time}  ")),
        Span::styled(
            format!("{} ", kind.glyph()),
            if stale { theme::dim() } else { style },
        ),
        Span::styled(e.entry.text.clone(), text_style),
    ])
}

fn render_footer(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let w = area.width.saturating_sub(1) as usize;
    // Keys for the selected game, then keys for the whole app. Without the
    // side panel, "enter" opens the details (and ranking) instead.
    let narrow = area.width < WIDE;
    let mut pairs = vec![("↑↓", if narrow { "choose" } else { "choose game" }, 1)];
    if narrow {
        pairs.extend([
            ("enter", "details", 1),
            ("1-9", "rank", 5),
            ("x", "skip", 6),
        ]);
    }
    pairs.push((
        "c",
        if cx.app.show_done {
            "hide done"
        } else {
            "show done"
        },
        7,
    ));
    // An expired sign-in is the one thing that needs the user: say so first.
    let expired = cx.expired();
    pairs.extend([
        (DIVIDER, "", 1),
        (
            "a",
            if expired { "sign in again" } else { "account" },
            if expired { 0 } else { 2 },
        ),
        ("g", "games", 4),
        ("l", "log", 6),
        (
            "p",
            if cx.app.farming.is_running() {
                "pause"
            } else {
                "carry on"
            },
            3,
        ),
        ("?", "help", 0),
        ("q", "quit", 0),
    ]);
    let line = hints(&pairs, w);
    let mut spans = vec![Span::raw(" ")];
    spans.extend(line.spans);
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hours_read_naturally() {
        assert_eq!(hours(0.0), "0.0h");
        assert_eq!(hours(5.24), "5.2h");
        assert_eq!(hours(12.4), "12h");
        assert_eq!(hours(1234.5), "1,235h");
    }

    #[test]
    fn countdowns_round_up_to_the_minute() {
        let now = Utc::now();
        assert_eq!(countdown(now, now), "now");
        assert_eq!(
            countdown(now + chrono::Duration::minutes(12), now),
            "in 12m"
        );
        assert_eq!(
            countdown(now + chrono::Duration::minutes(65), now),
            "in 1h 5m"
        );
    }
}
