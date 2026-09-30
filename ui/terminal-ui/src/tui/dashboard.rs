// The main screen: what's happening, two lines of totals, your games beside
// the cards that dropped this session, the latest event, and key hints.
//
// Every game has one state — farming (played alone, its cards dropping),
// building hours (played with others), queued, or done — drawn the same way
// in the games list and in its details.
//
// Text adapts to the space it has: each piece comes in a few lengths and the
// longest one that fits is drawn, so nothing is cut off mid-sentence. Only a
// game's or a card's name is ever shortened, with "…", and its details show
// it whole.

use std::time::Duration;

use chrono::{DateTime, Utc};
use farming::{EventKind, Mode, Status};
use preferences::Tier;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::{
    Ctx, LogEntry, MIN_HEIGHT, MIN_WIDTH,
    theme::{self, BAD, BUSY, GOOD},
    widgets::{
        DIVIDER, elapsed, fit, fit_right, fitted, flash_line, gauge, hints, keycap, panel, rule,
        selected_row, shorten, spread, width, wrap_text,
    },
};
use crate::viewmodel::{
    CardName, CardPrice, QueueEntry, Section, SessionCard, Summary, Value, card_price,
    session_cards, value_to_come,
};

/// Hours a game needs before its cards drop: what the farmer works to.
const HOURS_BEFORE_DROPS: f64 = 3.0;
/// From this width, the games and this session's cards sit side by side.
const SIDE_BY_SIDE: u16 = 90;
/// Width of the summary's labels, with their lead-in space: " This session  ".
const LABEL: usize = 15;
/// Log lines older than this fade in the dashboard strip.
const FRESH: Duration = Duration::from_secs(20);

/// Draws the dashboard; returns the games list's scroll offset to remember.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) -> usize {
    let [header, summary_area, body, strip, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(5),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);

    let summary = Summary::build(cx.session, cx.library, cx.order, cx.book, cx.wallet, cx.now);
    render_header(f, header, cx);
    render_summary(f, summary_area, cx, &summary);
    let offset = if area.width >= SIDE_BY_SIDE {
        let right = (area.width * 2 / 5).clamp(44, 72);
        let [games, session] =
            Layout::horizontal([Constraint::Min(30), Constraint::Length(right)]).areas(body);
        render_session(f, session, cx, &summary);
        render_games(f, games, cx)
    } else if body.height >= 14 {
        // Stacked: the games, then this session's newest cards.
        let below = (body.height / 3).clamp(4, 9);
        let [games, session] =
            Layout::vertical([Constraint::Min(6), Constraint::Length(below)]).areas(body);
        render_session(f, session, cx, &summary);
        render_games(f, games, cx)
    } else {
        render_games(f, body, cx)
    };
    render_strip(f, strip, cx);
    render_footer(f, footer, cx);
    offset
}

pub(super) fn too_small(f: &mut Frame<'_>, area: Rect) {
    let lines = vec![
        Line::styled("Make the window a little bigger", theme::bold()),
        Line::styled(
            format!(
                "needs {MIN_WIDTH}×{MIN_HEIGHT}, it's {}×{}",
                area.width, area.height
            ),
            theme::dim(),
        ),
    ];
    let r = Rect {
        y: area.y + (area.height / 2).saturating_sub(1),
        height: area.height.min(2),
        ..area
    };
    f.render_widget(Paragraph::new(lines).centered(), r);
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
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
        format!("{}h", thousands(h.round() as u64))
    }
}

/// "1,234".
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
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

/// "about 40 minutes", "about 17 hours", "about 5 days": an estimate, said
/// no more exactly than it's known.
fn about(d: Duration) -> String {
    let mins = d.as_secs().div_ceil(60);
    let (n, unit) = match mins {
        0..60 => (mins.max(1), "minute"),
        60..2_160 => ((mins + 30) / 60, "hour"),
        _ => ((mins + 720) / 1_440, "day"),
    };
    format!("about {n} {unit}{}", if n == 1 { "" } else { "s" })
}

/// "1 card", "3 cards".
fn cards(n: u64) -> String {
    if n == 1 {
        "1 card".to_owned()
    } else {
        format!("{} cards", thousands(n))
    }
}

/// "£1.45", or "£1.45+" when some cards aren't priced yet.
fn value(v: Value) -> String {
    format!("{}{}", v.money.grouped(), if v.partial { "+" } else { "" })
}

/// "2nd", "3rd", "11th".
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A price as the lists show it: the money, "…" while it's on its way, or
/// "—" when there's none to be had.
fn price(p: CardPrice) -> Span<'static> {
    match p {
        CardPrice::Worth(m) => Span::raw(m.grouped()),
        CardPrice::Waiting => dim("…"),
        CardPrice::None => dim("—"),
    }
}

// ── Header ───────────────────────────────────────────────────────────────────

fn render_header(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let w = area.width as usize;
    let pill = || vec![Span::styled(" steamcards ", theme::pill()), Span::raw("  ")];
    let rights = account_lines(cx);
    // Room for what's happening beside "appears offline" alone: a long game
    // name is shortened to it before it goes.
    let room = w.saturating_sub(width(&pill()) + 1 + width(&rights[2]));
    let lefts: Vec<Vec<Span<'static>>> = state_lines(cx, room)
        .into_iter()
        .map(|state| {
            let mut v = pill();
            v.extend(state);
            v
        })
        .collect();
    // Help goes first, then the next check, then the account's name, then
    // the game's name (shortened, then gone); "appears offline" goes last.
    const ORDER: [(usize, usize); 8] = [
        (0, 0),
        (0, 1),
        (1, 1),
        (1, 2),
        (2, 2),
        (3, 2),
        (3, 3),
        (4, 3),
    ];
    let chosen = ORDER
        .iter()
        .map(|&(l, r)| {
            (
                &lefts[l.min(lefts.len() - 1)],
                &rights[r.min(rights.len() - 1)],
            )
        })
        .find(|(l, r)| width(l) + 1 + width(r) <= w)
        .map(|(l, r)| (l.clone(), r.clone()));
    let (l, r) = chosen.unwrap_or_else(|| (pill(), Vec::new()));
    f.render_widget(fitted(vec![spread(l, r, w)], area), area);
}

/// What's happening, most to least detailed: with the game's name, then
/// with it shortened to `room`, then without it.
fn state_lines(cx: &Ctx<'_>, room: usize) -> Vec<Vec<Span<'static>>> {
    let say =
        |glyph: &str, style: Style, text: &str| Span::styled(format!("{glyph} {text}"), style);
    let then = |text: String| dim(format!(" · {text}"));

    if !cx.signed_in() {
        let head = say(theme::IDLE, theme::dim(), "not signed in");
        return vec![
            vec![head.clone(), then("press a to sign in".into())],
            vec![head],
        ];
    }
    if cx.expired() {
        let head = say(theme::FAILED, theme::fg(BAD), "sign-in expired");
        return vec![
            vec![head.clone(), then("press a to sign in again".into())],
            vec![head],
        ];
    }
    if !cx.app.farming.is_running() {
        let head = say(theme::PAUSED, theme::fg(BUSY), "paused");
        return vec![
            vec![head.clone(), then("press p to carry on".into())],
            vec![head],
        ];
    }
    let Some(s) = &cx.app.status else {
        return vec![vec![say(cx.spinner(), theme::fg(BUSY), "starting")]];
    };
    let next = |what: &str| {
        s.next_look
            .map(|at| then(format!("{what} {}", countdown(at, cx.now))))
    };
    let with = |head: Vec<Span<'static>>, tail: Option<Span<'static>>| {
        let mut all = vec![head.clone()];
        if let Some(t) = tail {
            let mut v = head.clone();
            v.push(t);
            all.insert(0, v);
        }
        all
    };
    match s.status {
        Status::Farming if s.mode == Some(Mode::Hours) => {
            let n = s.playing.len();
            let head = say(theme::HOURS, theme::fg(GOOD), "building hours");
            let on = if n == 1 {
                s.playing
                    .first()
                    .and_then(|&id| s.library.game(id))
                    .map_or_else(String::new, |g| format!(" on {}", g.name))
            } else {
                format!(" on {n} games")
            };
            let short = shorten(&on, room.saturating_sub(width(std::slice::from_ref(&head))));
            let mut lines = with(
                vec![head.clone(), Span::styled(on, theme::fg(GOOD))],
                next("ready"),
            );
            if short.chars().count() > 8 {
                lines.push(vec![head.clone(), Span::styled(short, theme::fg(GOOD))]);
            }
            lines.push(vec![head]);
            lines
        }
        Status::Farming => {
            let game = s.playing.first().and_then(|&id| s.library.game(id));
            let head = say(theme::SIGNED_IN, theme::fg(GOOD), "farming");
            let Some(game) = game else {
                return vec![vec![head]];
            };
            let named = vec![
                head.clone(),
                Span::raw(" "),
                Span::styled(game.name.clone(), theme::strong(GOOD)),
            ];
            let short = shorten(
                &game.name,
                room.saturating_sub(width(std::slice::from_ref(&head)) + 1),
            );
            let mut lines = with(named, next("next check"));
            if short.chars().count() > 6 {
                lines.push(vec![
                    head.clone(),
                    Span::raw(" "),
                    Span::styled(short, theme::strong(GOOD)),
                ]);
            }
            lines.push(vec![head]);
            lines
        }
        Status::Checking => vec![vec![say(
            cx.spinner(),
            theme::fg(BUSY),
            "reading your badges",
        )]],
        Status::Blocked => {
            let head = say(theme::PAUSED, theme::fg(BUSY), "waiting");
            // It's done: farming carries on in a while.
            if s.next_look.is_some() {
                return with(vec![head], next("farming again"));
            }
            let what = s.blocked_by.and_then(|id| s.library.game(id)).map_or_else(
                || "your Steam account is in use on another device".to_owned(),
                |g| format!("{} is being played on another device", g.name),
            );
            vec![
                vec![head.clone(), then(what)],
                vec![head.clone(), then("played elsewhere".into())],
                vec![head],
            ]
        }
        Status::Idle => with(
            vec![say(theme::IDLE, theme::dim(), "nothing to farm")],
            next("looks again"),
        ),
        Status::Error => {
            let note = if s.note.is_empty() {
                "something went wrong".to_owned()
            } else {
                s.note.clone()
            };
            let head = say(theme::FAILED, theme::fg(BAD), &note);
            let mut lines = with(vec![head], next("trying again"));
            lines.push(vec![say(
                theme::FAILED,
                theme::fg(BAD),
                "trouble reaching Steam",
            )]);
            lines
        }
    }
}

/// Who's signed in and how they show to friends, then help: most to least
/// detailed.
fn account_lines(cx: &Ctx<'_>) -> Vec<Vec<Span<'static>>> {
    let name = cx
        .account
        .map(|a| a.name.clone())
        .filter(|n| !n.is_empty() && cx.signed_in());
    let online = if cx.prefs.appear_online {
        "online"
    } else {
        "appears offline"
    };
    let help = vec![Span::raw("   "), keycap("?"), dim(" help ")];
    let mut full = Vec::new();
    if let Some(n) = &name {
        full.push(Span::raw(n.clone()));
        full.push(dim(" · "));
    }
    full.push(dim(online));
    let mut with_help = full.clone();
    with_help.extend(help);
    vec![with_help, full, vec![dim(online)], Vec::new()]
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

// ── Summary ──────────────────────────────────────────────────────────────────

/// Two lines: this session and what's to go; then every game, and what it
/// will all be worth.
fn render_summary(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, s: &Summary) {
    let w = area.width as usize;
    let label = |text: &str| dim(format!(" {}", fit(text, LABEL - 1)));
    let reading = cx.library.is_empty();

    // This session: its cards, and what they're worth.
    let session: Vec<Span<'static>> = if s.session_cards == 0 {
        vec![dim("no cards yet")]
    } else {
        let mut v = vec![Span::styled(cards(s.session_cards as u64), theme::bold())];
        if let Some(worth) = s.session_value {
            v.push(dim(" · "));
            v.push(Span::styled(value(worth), theme::strong(GOOD)));
        }
        v
    };
    // What's to go: most to least detailed.
    let to_go: Vec<Vec<Span<'static>>> = if reading {
        vec![vec![dim("reading your badges")]]
    } else if s.cards_to_go == 0 {
        vec![vec![Span::styled("nothing left to farm", theme::fg(GOOD))]]
    } else {
        let n = Span::styled(cards(u64::from(s.cards_to_go)), theme::bold());
        let games = dim(format!(
            " · {} game{}",
            s.games_to_go,
            if s.games_to_go == 1 { "" } else { "s" }
        ));
        let time = s.time_to_go.map(|d| dim(format!(" · {}", about(d))));
        let mut out = Vec::new();
        let mut full = vec![n.clone(), games];
        full.extend(time.clone());
        out.push(full);
        let mut short = vec![n.clone()];
        short.extend(time);
        out.push(short);
        out.push(vec![n]);
        out
    };
    let first = to_go
        .iter()
        .map(|t| {
            let mut v = vec![label("This session")];
            v.extend(session.clone());
            v.push(Span::raw("     "));
            v.push(dim("To go  "));
            v.extend(t.iter().cloned());
            v
        })
        .chain(to_go.iter().map(|t| {
            let mut v = vec![Span::raw(" ")];
            v.extend(session.clone());
            v.push(dim("  ·  to go "));
            v.extend(t.iter().cloned());
            v
        }))
        .find(|v| width(v) <= w)
        .unwrap_or_else(|| {
            let mut v = vec![Span::raw(" ")];
            v.extend(session.clone());
            v
        });

    // Every game: the drops received of all there are, and the value when
    // every card has dropped.
    let counts = Span::raw(format!(
        "{} of {}",
        thousands(u64::from(s.all_received)),
        thousands(u64::from(s.all_total))
    ));
    let worth: Vec<Span<'static>> = s
        .when_done
        .map(|v| {
            vec![
                dim(" · when done ≈ "),
                Span::styled(value(v), theme::strong(GOOD)),
            ]
        })
        .unwrap_or_default();
    let second = if reading {
        vec![label("All games"), dim("…")]
    } else {
        let tail = |with_worth: bool| {
            let mut v = vec![Span::raw("  "), counts.clone()];
            if with_worth {
                v.extend(worth.iter().cloned());
            }
            v
        };
        let frac = if s.all_total == 0 {
            0.0
        } else {
            f64::from(s.all_received) / f64::from(s.all_total)
        };
        [true, false]
            .into_iter()
            .find_map(|with_worth| {
                let t = tail(with_worth);
                let room = w.saturating_sub(LABEL + width(&t) + 1);
                (room >= 10).then(|| {
                    let mut v = vec![label("All games")];
                    v.extend(gauge(frac, room.min(60), theme::fg(GOOD)));
                    v.extend(t);
                    v
                })
            })
            .or_else(|| {
                let mut v = vec![label("All games"), counts.clone()];
                v.extend(worth.iter().cloned());
                (width(&v) <= w).then_some(v)
            })
            .unwrap_or_else(|| vec![label("All games"), counts.clone()])
    };
    f.render_widget(
        fitted(vec![Line::from(first), Line::from(second)], area),
        area,
    );
}

// ── Games ────────────────────────────────────────────────────────────────────

/// Column widths for the games list, fitted to its panel: the name takes what
/// the counts and the value leave.
struct GameCols {
    rank: usize,
    name: usize,
    value: usize,
}

impl GameCols {
    /// "▶ " before each game.
    const MARK: usize = 2;
    /// " 12/15".
    const CARDS: usize = 7;

    fn new(w: usize, ranked: bool) -> Self {
        let rank = if ranked { 4 } else { 0 };
        let value = if w >= Self::MARK + rank + 12 + Self::CARDS + 10 {
            10
        } else {
            0
        };
        Self {
            rank,
            name: w.saturating_sub(Self::MARK + rank + Self::CARDS + value),
            value,
        }
    }
}

/// Draws the games list; returns its scroll offset.
fn render_games(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) -> usize {
    let q = cx.queue;
    let mut block = panel(Line::styled(" Games ", theme::heading()));
    let inner = block.inner(area);
    if q.is_empty() {
        f.render_widget(block, area);
        render_games_empty(f, inner, cx);
        return 0;
    }
    let entries: Vec<&QueueEntry> = q
        .entries()
        .filter(|e| cx.app.show_done || e.section() != Section::Done)
        .collect();
    let to_go = q
        .entries()
        .filter(|e| e.wanted && matches!(e.section(), Section::Priority | Section::Indifferent))
        .count();
    block = block.title_top(Line::from(dim(format!(" {to_go} to go "))).right_aligned());

    let w = inner.width as usize;
    let ranked = entries.iter().any(|e| matches!(e.tier, Tier::Priority(_)));
    let cols = GameCols::new(w, ranked);
    let [head_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(inner);
    let h = list_area.height as usize;

    let mut lines = Vec::with_capacity(entries.len());
    let mut sel = None;
    for (i, e) in entries.iter().enumerate() {
        let row = game_row(e, &cols, cx);
        if cx.app.selected == Some(e.game.app_id) {
            sel = Some(i);
            lines.push(selected_row(row, w));
        } else {
            lines.push(row);
        }
    }

    // Scroll just enough to keep the chosen game in view.
    let max = lines.len().saturating_sub(h);
    let mut off = cx.app.queue_offset.min(max);
    if let Some(s) = sel {
        if s < off {
            off = s;
        }
        if s >= off + h {
            off = s + 1 - h;
        }
    }
    let (above, below) = (off, lines.len().saturating_sub(off + h));
    let more = match (above, below) {
        (0, 0) => None,
        (0, b) => Some(format!(" {b} more ↓ ")),
        (a, 0) => Some(format!(" {a} more ↑ ")),
        (a, b) => Some(format!(" {a} ↑ · {b} ↓ ")),
    };
    if let Some(more) = more {
        block = block.title_bottom(Line::from(dim(more)).right_aligned());
    }
    if cx.app.show_done {
        block = block.title_bottom(Line::from(vec![dim(" "), keycap("c"), dim(" hide done ")]));
    }
    f.render_widget(block, area);
    f.render_widget(fitted(vec![column_header(&cols)], head_area), head_area);
    f.render_widget(fitted(lines, list_area).scroll((off as u16, 0)), list_area);
    off
}

fn column_header(c: &GameCols) -> Line<'static> {
    let mut s = " ".repeat(GameCols::MARK + c.rank);
    s.push_str(&fit("GAME", c.name));
    s.push_str(&fit_right("CARDS", GameCols::CARDS));
    if c.value > 0 {
        s.push_str(&fit_right("TO COME", c.value));
    }
    Line::styled(s, theme::dim())
}

fn game_row(e: &QueueEntry, c: &GameCols, cx: &Ctx<'_>) -> Line<'static> {
    let state = State::of(e);
    let done = state == State::Done;
    let skipped = e.tier == Tier::Skip && !done;
    let muted = done || skipped || !e.wanted;
    let text = if muted { theme::dim() } else { theme::plain() };

    let mut s = vec![if skipped {
        Span::styled(fit(theme::SKIPPED, GameCols::MARK), theme::fg(BAD))
    } else {
        Span::styled(fit(state.symbol(), GameCols::MARK), state.style())
    }];
    if c.rank > 0 {
        s.push(match e.tier {
            Tier::Priority(n) if !done => {
                Span::styled(fit(&format!("#{n}"), c.rank), theme::strong(BUSY))
            }
            _ => Span::raw(" ".repeat(c.rank)),
        });
    }
    s.push(Span::styled(
        fit(&shorten(&e.game.name, c.name.saturating_sub(1)), c.name),
        text,
    ));
    s.push(Span::styled(
        fit_right(
            &format!("{}/{}", e.game.drops.received, e.game.drops.total()),
            GameCols::CARDS,
        ),
        text,
    ));
    if c.value > 0 {
        // A game that won't be farmed has nothing to come.
        let v = value_to_come(&e.game, cx.book, cx.wallet).filter(|_| !muted);
        let cell = v.map(price).unwrap_or_else(|| Span::raw(""));
        let style = if muted { theme::dim() } else { cell.style };
        s.push(Span::styled(fit_right(&cell.content, c.value), style));
    }
    Line::from(s)
}

fn render_games_empty(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
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

// ── This session ─────────────────────────────────────────────────────────────

fn render_session(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, s: &Summary) {
    let mut block = panel(Line::styled(" This session ", theme::heading()));
    if s.session_cards > 0 {
        let mut title = vec![dim(format!(" {}", cards(s.session_cards as u64)))];
        if let Some(v) = s.session_value {
            title.push(dim(format!(" · {} ", value(v))));
        } else {
            title.push(dim(" "));
        }
        block = block.title_top(Line::from(title).right_aligned());
    }
    let inner = block.inner(area);
    let w = inner.width as usize;
    let h = inner.height as usize;

    let list = session_cards(cx.session, cx.library, cx.book, cx.wallet);
    if list.is_empty() {
        f.render_widget(block, area);
        let text = "No cards yet. The first usually drops within half an hour of a game \
                    being played on its own.";
        let lines: Vec<Line<'_>> = wrap_text(text, w)
            .into_iter()
            .take(h)
            .map(|l| Line::styled(l, theme::dim()))
            .collect();
        f.render_widget(fitted(lines, inner), inner);
        return;
    }
    if list.len() > h {
        block = block
            .title_bottom(Line::from(dim(format!(" {} earlier ", list.len() - h))).right_aligned());
    }
    f.render_widget(block, area);

    // Time, game, card, price: the game gives way before the card does.
    const TIME: usize = 6;
    const PRICE: usize = 8;
    let shown = &list[..list.len().min(h)];
    let longest = shown
        .iter()
        .map(|c| c.game.chars().count())
        .max()
        .unwrap_or(0);
    let game_w = longest.min(18).min(w.saturating_sub(TIME + PRICE + 10) / 2) + 2;
    let card_w = w.saturating_sub(TIME + game_w + PRICE);
    let lines: Vec<Line<'_>> = shown
        .iter()
        .map(|c| session_row(c, cx, game_w, card_w, PRICE))
        .collect();
    f.render_widget(fitted(lines, inner), inner);
}

fn session_row(
    c: &SessionCard,
    cx: &Ctx<'_>,
    game_w: usize,
    card_w: usize,
    price_w: usize,
) -> Line<'static> {
    let time = c.at.with_timezone(&cx.zone).format("%H:%M").to_string();
    let mut s = vec![
        dim(fit(&time, 6)),
        Span::raw(fit(&shorten(&c.game, game_w.saturating_sub(2)), game_w)),
    ];
    let room = card_w.saturating_sub(1);
    match &c.card {
        CardName::Named { name, foil, copy } => {
            let star = if *foil { "★ " } else { "" };
            let spare = match copy {
                Some(n) if *n >= 2 => format!(", {}", ordinal(*n)),
                _ => String::new(),
            };
            let name_room = room.saturating_sub(star.chars().count() + spare.chars().count());
            let mut cell = Vec::new();
            if *foil {
                cell.push(Span::styled(star, theme::fg(theme::ACCENT)));
            }
            cell.push(Span::raw(shorten(name, name_room)));
            cell.push(dim(spare));
            let used = width(&cell);
            cell.push(Span::raw(" ".repeat(card_w.saturating_sub(used))));
            s.extend(cell);
        }
        CardName::Finding => {
            s.push(Span::styled(format!("{} ", cx.spinner()), theme::fg(BUSY)));
            s.push(dim(fit("which card?", card_w.saturating_sub(2))));
        }
        CardName::Unknown => s.push(dim(fit("a card", card_w))),
    }
    let p = price(c.price);
    s.push(Span::styled(fit_right(&p.content, price_w), p.style));
    Line::from(s)
}

// ── Details (a pop-up) ───────────────────────────────────────────────────────

/// Everything about one game — what it's doing, its hours and drops, and its
/// set with how many of each card you have and what each is worth.
pub(super) fn detail_info(e: &QueueEntry, cx: &Ctx<'_>, w: usize) -> Vec<Line<'static>> {
    let g = &e.game;
    let state = State::of(e);
    let mut out = Vec::new();

    let others = e
        .playing
        .map(|_| {
            cx.queue
                .entries()
                .filter(|x| x.playing.is_some() && x.game.app_id != g.app_id)
                .count()
        })
        .unwrap_or(0);
    let first = cx
        .app
        .status
        .as_ref()
        .and_then(|s| s.order.first().copied());
    let (headline, meaning): (Span<'static>, String) = match state {
        State::Farming => (
            Span::styled(format!("{} Farming now", theme::FARMING), state.style()),
            "Played on its own, so its cards can drop.".to_owned(),
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
            "Never farmed.".to_owned(),
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

    let field = |label: &str, value: Vec<Span<'static>>| {
        let mut spans = vec![dim(fit(label, 8))];
        spans.extend(value);
        Line::from(spans)
    };
    let mut played = vec![Span::raw(format!("{} on record", hours(g.hours)))];
    if g.hours < HOURS_BEFORE_DROPS && g.has_drops_left() {
        played.push(dim(" · cards drop from 3h"));
    }
    out.push(field("Hours", played));
    let mut drops = if g.has_drops_left() {
        vec![
            Span::raw(format!("{} of {}", g.drops.received, g.drops.total())),
            dim(format!(" · {} to come", g.drops.remaining)),
        ]
    } else {
        vec![Span::styled(
            format!("all {} dropped", g.drops.total()),
            theme::fg(GOOD),
        )]
    };
    if let Some(CardPrice::Worth(m)) = value_to_come(g, cx.book, cx.wallet) {
        drops.push(dim(format!(" · ≈ {}", m.grouped())));
    }
    out.push(field("Drops", drops));

    let Some(set) = cx.sets.set(g.app_id).filter(|s| !s.is_empty()) else {
        out.push(field(
            "Set",
            vec![dim("not read yet: its card page is read once it's farmed")],
        ));
        return out;
    };
    let spares = set.spares();
    let mut collected = vec![Span::raw(format!(
        "{} of {} cards",
        set.collected(),
        set.len()
    ))];
    if spares > 0 {
        collected.push(dim(format!(
            " · {spares} spare{}",
            if spares == 1 { "" } else { "s" }
        )));
    }
    out.push(field("Set", collected));
    out.push(Line::default());

    // The set: each card, how many you have, and what it's worth.
    let name_w = set
        .cards()
        .iter()
        .map(|c| c.name.chars().count())
        .max()
        .unwrap_or(0)
        .min(w.saturating_sub(2 + 5 + 9));
    for c in set.cards() {
        let count = match c.owned {
            0 => dim(fit_right("—", 5)),
            n => Span::raw(fit_right(&format!("×{n}"), 5)),
        };
        let p = price(card_price(g.app_id, &c.name, cx.book, cx.wallet));
        out.push(Line::from(vec![
            Span::raw("  "),
            Span::raw(fit(&shorten(&c.name, name_w), name_w)),
            count,
            Span::styled(fit_right(&p.content, 9), p.style),
        ]));
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

// ── Strip and footer ─────────────────────────────────────────────────────────

/// The latest event (not a routine look), or a just-now confirmation. (While
/// a pop-up is open, it draws the confirmation itself, where the pop-up can't
/// cover it.)
fn render_strip(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let line = match cx.app.flash.as_ref().filter(|_| cx.app.overlay.is_none()) {
        Some((msg, _)) => flash_line(&shorten(msg, (area.width as usize).saturating_sub(4))),
        None => cx
            .app
            .log
            .iter()
            .rev()
            .find(|e| e.kind != EventKind::Progress)
            .map(|e| log_line(e, true, area.width as usize))
            .unwrap_or_default(),
    };
    f.render_widget(fitted(vec![line], area), area);
}

/// A log entry in `w` columns: time, icon, text, the text shortened with
/// "…" when it doesn't fit. `fade` greys out older entries.
pub(super) fn log_line(e: &LogEntry, fade: bool, w: usize) -> Line<'static> {
    let (icon, style) = match e.kind {
        EventKind::Dropped | EventKind::Identified => (theme::DONE, theme::fg(GOOD)),
        EventKind::Playing => (theme::FARMING, theme::fg(GOOD)),
        EventKind::Switched => ("↻", theme::fg(theme::LINK)),
        EventKind::Warning => ("!", theme::fg(BUSY)),
        EventKind::Error => (theme::FAILED, theme::fg(BAD)),
        EventKind::Info | EventKind::Progress => ("·", theme::dim()),
    };
    let stale = fade && e.received.elapsed() > FRESH;
    let text_style = match e.kind {
        _ if stale => theme::dim(),
        EventKind::Progress => theme::dim(),
        EventKind::Dropped | EventKind::Identified => theme::fg(GOOD),
        EventKind::Error => theme::fg(BAD),
        _ => theme::plain(),
    };
    let time = if fade {
        e.at.format("%H:%M")
    } else {
        e.at.format("%H:%M:%S")
    };
    let time = format!(" {time}  ");
    let room = w.saturating_sub(time.chars().count() + icon.chars().count() + 1);
    Line::from(vec![
        dim(time),
        Span::styled(format!("{icon} "), if stale { theme::dim() } else { style }),
        Span::styled(shorten(&e.text, room), text_style),
    ])
}

fn render_footer(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let w = area.width.saturating_sub(1) as usize;
    // An expired sign-in is the one thing that needs the user: say so first.
    let expired = cx.expired();
    let pairs = [
        ("↑↓", "choose", 1),
        ("enter", "details", 1),
        ("1-9", "rank", 5),
        ("x", "skip", 6),
        (
            "c",
            if cx.app.show_done {
                "hide done"
            } else {
                "show done"
            },
            8,
        ),
        (DIVIDER, "", 1),
        (
            "a",
            if expired { "sign in again" } else { "account" },
            if expired { 0 } else { 2 },
        ),
        ("g", "games", 4),
        ("l", "log", 7),
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
    ];
    let line = hints(&pairs, w);
    let mut spans = vec![Span::raw(" ")];
    spans.extend(line.spans);
    f.render_widget(fitted(vec![Line::from(spans)], area), area);
}

#[cfg(test)]
mod tests {
    use game::Game;

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

    #[test]
    fn estimates_say_about_how_long_no_more_exactly() {
        let mins = |m: u64| Duration::from_secs(m * 60);
        assert_eq!(about(mins(0)), "about 1 minute");
        assert_eq!(about(mins(40)), "about 40 minutes");
        assert_eq!(about(mins(95)), "about 2 hours");
        assert_eq!(about(mins(17 * 60 + 20)), "about 17 hours");
        assert_eq!(about(mins(36 * 60)), "about 2 days");
        assert_eq!(about(mins(117 * 60)), "about 5 days");
    }

    #[test]
    fn copies_are_counted_in_words() {
        assert_eq!(ordinal(2), "2nd");
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(4), "4th");
        assert_eq!(ordinal(11), "11th");
        assert_eq!(ordinal(22), "22nd");
    }

    #[test]
    fn a_game_reads_as_its_state() {
        use game::test_support::game;
        let entry = |remaining: u32, playing: Option<Mode>| QueueEntry {
            game: game(1, 5.0, 1, remaining),
            tier: Tier::Indifferent,
            playing,
            wanted: true,
        };
        assert_eq!(State::of(&entry(2, Some(Mode::Cards))), State::Farming);
        assert_eq!(State::of(&entry(2, Some(Mode::Hours))), State::Hours);
        assert_eq!(State::of(&entry(2, None)), State::Queued);
        assert_eq!(State::of(&entry(0, None)), State::Done);
    }

    #[test]
    fn a_game_with_nothing_to_come_shows_no_value() {
        let g = Game {
            app_id: 1,
            name: "Done".into(),
            hours: 5.0,
            drops: game::CardDrops {
                received: 3,
                remaining: 0,
            },
            badge_level: 0,
        };
        assert_eq!(value_to_come(&g, &price::PriceBook::default(), None), None);
    }
}
