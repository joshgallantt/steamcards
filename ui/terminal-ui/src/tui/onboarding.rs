// First run, full screen and one step at a time: a welcome, signing in with
// the Steam app, picking games, starting to farm — with the steps along the
// top. Signing out comes back to the sign-in step.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Padding, Paragraph},
};

use super::{
    CONTINUE_ROW, Ctx, format, overlays,
    theme::{self, ACCENT, BAD, BUSY, GOOD},
    widgets::{
        centered, first_fit, fit, flash_line, hints, keycap, scroll, selected_row, spread, width,
        wrap_list, wrap_text,
    },
};
use crate::viewmodel::{GameRow, Step};

/// The card's width on roomy windows.
const CARD_W: u16 = 80;
/// Borders and padding around the card's text: rows, then columns.
const CARD_PAD: (u16, u16) = (4, 8);

/// What a step shows: fixed lines, a list that scrolls to keep `focus` in
/// view, and fixed lines under it.
struct Card {
    title: &'static str,
    /// The card's top-right corner, e.g. how many games are picked.
    note: String,
    head: Vec<Line<'static>>,
    list: Vec<Line<'static>>,
    focus: usize,
    tail: Vec<Line<'static>>,
    keys: Vec<(&'static str, &'static str, u8)>,
}

impl Card {
    fn new(title: &'static str, head: Vec<Line<'static>>) -> Self {
        Self {
            title,
            note: String::new(),
            head,
            list: Vec::new(),
            focus: 0,
            tail: Vec::new(),
            keys: Vec::new(),
        }
    }
}

pub(super) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, step: Step) {
    let [header, body, strip, footer] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(8),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(area);
    render_steps(f, header, step);

    let w = CARD_W.min(body.width.saturating_sub(2));
    let text_w = w.saturating_sub(CARD_PAD.1) as usize;
    let room = body.height.saturating_sub(CARD_PAD.0) as usize;
    let card = match step {
        Step::Welcome => welcome(text_w, room),
        Step::SignIn => sign_in(cx, text_w, room),
        Step::Games => games(cx, text_w, room),
        Step::Start => start(cx, text_w, room),
    };

    let lines = card.head.len() + card.list.len() + card.tail.len();
    let r = centered(body, w, (lines as u16 + CARD_PAD.0).min(body.height));
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::modal_border())
        .title(Line::styled(format!(" {} ", card.title), theme::brand()))
        .padding(Padding::new(3, 3, 1, 1));
    if !card.note.is_empty() {
        block = block.title(Line::from(dim(format!(" {} ", card.note))).right_aligned());
    }
    let inner = block.inner(r);
    f.render_widget(block, r);
    let [head, list, tail] = Layout::vertical([
        Constraint::Length(card.head.len() as u16),
        Constraint::Min(0),
        Constraint::Length(card.tail.len() as u16),
    ])
    .areas(inner);
    let off = scroll(card.focus, card.list.len(), list.height as usize);
    f.render_widget(Paragraph::new(card.head), head);
    f.render_widget(Paragraph::new(card.list).scroll((off as u16, 0)), list);
    f.render_widget(Paragraph::new(card.tail), tail);

    // A pop-up (signing in, help) draws the message itself.
    if let Some((msg, _)) = cx.app.flash.as_ref().filter(|_| cx.app.overlay.is_none()) {
        f.render_widget(Paragraph::new(flash_line(msg)), strip);
    }
    let mut spans = vec![Span::raw(" ")];
    spans.extend(hints(&card.keys, footer.width.saturating_sub(1) as usize).spans);
    f.render_widget(Paragraph::new(Line::from(spans)), footer);
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

fn name(s: Step) -> &'static str {
    match s {
        Step::Welcome => "Welcome",
        Step::SignIn => "Sign in",
        Step::Games => "Games",
        Step::Start => "Start",
    }
}

/// "steamcards   ✓ Welcome › 2 Sign in › 3 Games › 4 Start", or shorter.
fn render_steps(f: &mut Frame<'_>, area: Rect, step: Step) {
    let at = Step::ALL.iter().position(|s| *s == step).unwrap_or(0);
    let mut full = vec![
        Span::styled(" steamcards ", theme::pill()),
        Span::raw("   "),
    ];
    for (i, s) in Step::ALL.into_iter().enumerate() {
        if i > 0 {
            full.push(dim(" › "));
        }
        full.push(match i.cmp(&at) {
            std::cmp::Ordering::Less => {
                Span::styled(format!("{} {}", theme::DONE, name(s)), theme::fg(GOOD))
            }
            std::cmp::Ordering::Equal => {
                Span::styled(format!("{} {}", i + 1, name(s)), theme::strong(ACCENT))
            }
            std::cmp::Ordering::Greater => dim(format!("{} {}", i + 1, name(s))),
        });
    }
    let short = vec![
        Span::styled(" steamcards ", theme::pill()),
        Span::raw("   "),
        dim(format!("Step {} of {} · ", at + 1, Step::ALL.len())),
        Span::styled(name(step), theme::strong(ACCENT)),
    ];
    let line = first_fit(vec![full, short.clone()], area.width as usize).unwrap_or(short);
    f.render_widget(Paragraph::new(Line::from(line)), area);
}

/// "1  Sign in…": a numbered line.
fn numbered(n: &str, text: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{n}  "), theme::strong(BUSY)),
        Span::raw(text.to_owned()),
    ])
}

fn paragraph(text: &str, w: usize) -> Vec<Line<'static>> {
    wrap_text(text, w).into_iter().map(Line::raw).collect()
}

// ── Welcome ──────────────────────────────────────────────────────────────────

fn welcome(w: usize, room: usize) -> Card {
    let mut head = paragraph(
        "steamcards plays your Steam games in the background, so their trading \
         cards drop while you get on with your day. Nothing is installed or \
         launched: Steam is simply told what's being played.",
        w,
    );
    head.extend([
        Line::default(),
        numbered("1", "Sign in with the Steam app on your phone"),
        numbered("2", "Pick the games you want cards from first"),
        numbered("3", "Start farming, and leave it running"),
        Line::default(),
    ]);
    // Said before anything starts, so starting is agreeing to it.
    head.extend(
        wrap_text(
            "steamcards is unofficial: Valve doesn't make or support it. Farming \
             cards is at your own risk.",
            w,
        )
        .into_iter()
        .map(|line| Line::styled(line, theme::dim())),
    );
    head.extend([
        Line::default(),
        Line::from(vec![
            Span::raw("Press "),
            keycap("enter"),
            Span::raw(" to agree and begin."),
        ]),
    ]);
    if head.len() > room {
        head.retain(|l| width(&l.spans) > 0);
    }
    let mut card = Card::new("Welcome to steamcards", head);
    card.keys = vec![
        ("enter", "agree and begin", 0),
        ("?", "help", 2),
        ("q", "quit", 1),
    ];
    card
}

// ── Sign in ──────────────────────────────────────────────────────────────────

fn sign_in(cx: &Ctx<'_>, w: usize, room: usize) -> Card {
    let cursor = cx.app.setup.cursor;
    let mut head = paragraph(
        "Scan a QR code with the Steam app on your phone and approve it there. \
         No password is typed in here.",
        w,
    );
    head.push(Line::default());
    let mut left = vec![
        Span::raw(" "),
        Span::styled(fit("Steam", 9), theme::steam()),
    ];
    left.extend(overlays::account_badge(cx));
    let right = if cursor == 0 {
        vec![Span::raw(format!("{} › ", sign_in_label(cx)))]
    } else {
        Vec::new()
    };
    let row = spread(left, right, w);
    head.push(if cursor == 0 {
        selected_row(row, w)
    } else {
        row
    });
    head.push(Line::default());
    let row = if cx.app.onboarding.can_continue() {
        Line::styled(" Continue →", theme::strong(GOOD))
    } else {
        Line::from(vec![
            Span::styled(" Continue →", theme::dim()),
            dim("  sign in first"),
        ])
    };
    head.push(if cursor == CONTINUE_ROW {
        selected_row(row, w)
    } else {
        row
    });
    if head.len() > room {
        head.retain(|l| width(&l.spans) > 0);
    }

    let mut card = Card::new("Sign in to Steam", head);
    let enter = if cursor == 0 {
        sign_in_label(cx)
    } else {
        "continue"
    };
    card.keys = vec![
        ("↑↓", "select", 2),
        ("enter", enter, 0),
        ("→", "continue", 1),
        ("←", "back", 3),
        ("?", "help", 4),
        ("q", "quit", 1),
    ];
    card
}

/// What enter does to the account.
fn sign_in_label(cx: &Ctx<'_>) -> &'static str {
    if cx.signed_in() {
        "sign in again"
    } else {
        "sign in"
    }
}

// ── Games ────────────────────────────────────────────────────────────────────

fn games(cx: &Ctx<'_>, w: usize, room: usize) -> Card {
    let app = cx.app;
    let rows = app.picks();
    let cursor = app.setup.cursor.min(rows.len().saturating_sub(1));
    let only = cx.prefs.only_priority;

    let mut head = paragraph(
        if only {
            "The games you pick are farmed first, in the order you pick them."
        } else {
            "The games you pick are farmed first, in the order you pick them. \
             Skip this and steamcards farms the games closest to dropping first."
        },
        w,
    );
    head.push(Line::default());
    // Leave the list five rows at least; the explanation goes first.
    if head.len() + 5 > room {
        head.clear();
    }

    let mut list: Vec<Line<'static>> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let row = pick_row(r, w);
            if i == cursor {
                selected_row(row, w)
            } else {
                row
            }
        })
        .collect();
    // Where the library is at: under the games, or in their place.
    let mut tail = Vec::new();
    let status = library_status(cx, w);
    if rows.is_empty() {
        list.extend(status);
    } else if !status.is_empty() {
        tail.push(Line::default());
        tail.extend(status);
    }
    if only {
        tail.push(Line::default());
        tail.extend(
            wrap_text(
                "\"Only priority\" is on: only the games you pick are farmed. Press o to \
                 turn it off.",
                w,
            )
            .into_iter()
            .map(|l| Line::styled(l, theme::fg(BUSY))),
        );
    }

    let picked = cx.prefs.priority_games.len();
    let mut card = Card::new("Pick your games", head);
    card.note = match picked {
        0 => String::new(),
        n => format!("{n} picked"),
    };
    card.list = list;
    card.focus = cursor;
    card.tail = tail;
    let on_a_pick = rows.get(cursor).is_some_and(|r| r.rank.is_some());
    card.keys = vec![
        ("↑↓", "select", 2),
        ("space", if on_a_pick { "unpick" } else { "pick" }, 0),
        ("o", "only priority", 5),
        ("r", "read again", 6),
        ("enter", if picked == 0 { "skip" } else { "continue" }, 0),
        ("←", "back", 2),
        ("?", "help", 4),
        ("q", "quit", 1),
    ];
    card
}

/// Reading, failed or empty: the library, when it's not simply there, in
/// lines of `w` columns; none when it is.
pub(super) fn library_status(cx: &Ctx<'_>, w: usize) -> Vec<Line<'static>> {
    let library = &cx.app.library;
    let say = |text: &str, style| -> Vec<Line<'static>> {
        wrap_text(text, w)
            .into_iter()
            .map(|l| Line::styled(l, style))
            .collect()
    };
    if library.is_loading() {
        return vec![Line::from(vec![
            Span::styled(format!("{} ", cx.spinner()), theme::fg(BUSY)),
            Span::raw("Reading your badges for games with cards left…"),
        ])];
    }
    if let Some(e) = library.error() {
        return say(
            &format!("Couldn't read your badges: {e}. Press r to try again, or skip this."),
            theme::fg(BAD),
        );
    }
    match library.library() {
        None => say("Press r to read your badges.", theme::dim()),
        Some(l) if l.drops_left() == 0 => say(
            "No games with cards left to drop right now. Skip this: steamcards looks \
             again every few hours.",
            theme::dim(),
        ),
        Some(_) => Vec::new(),
    }
}

/// "#1  Portal 2                     3 cards to drop · 5.2h"
fn pick_row(r: &GameRow, w: usize) -> Line<'static> {
    let badge = match r.rank {
        Some(n) => Span::styled(fit(&format!("#{n}"), 4), theme::strong(BUSY)),
        None => Span::raw("    "),
    };
    let left = vec![Span::raw(" "), badge, Span::raw(r.name.clone())];
    let right = game_facts(r, w.saturating_sub(1 + 4 + 14));
    spread(left, right, w)
}

/// A game's drops to come and hours, as much as fits in `room`, for here and
/// the games pop-up.
pub(super) fn game_facts(r: &GameRow, room: usize) -> Vec<Span<'static>> {
    let left = match r.drops.remaining {
        0 => "done".to_owned(),
        1 => "1 card to drop".to_owned(),
        n => format!("{n} cards to drop"),
    };
    let played = format::hours(r.hours);
    first_fit(
        vec![
            vec![dim(format!("  {left} · {played} played "))],
            vec![dim(format!("  {left} · {played} "))],
            vec![dim(format!("  {left} "))],
        ],
        room,
    )
    .unwrap_or_default()
}

// ── Start ────────────────────────────────────────────────────────────────────

fn start(cx: &Ctx<'_>, w: usize, room: usize) -> Card {
    const LABEL: usize = 10;
    let mut head = Vec::new();
    let mut spans = vec![
        Span::raw(" "),
        Span::styled(fit("Steam", LABEL), theme::steam()),
    ];
    spans.extend(overlays::account_badge(cx));
    head.push(Line::from(spans));

    let games: Vec<String> = cx
        .prefs
        .priority_games
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let name = cx
                .app
                .known_library()
                .game(id)
                .map_or_else(|| format!("App {id}"), |g| g.name.clone());
            format!("#{} {name}", i + 1)
        })
        .collect();
    let label = Span::styled(fit(" Games", LABEL + 1), theme::bold());
    let only = cx.prefs.only_priority;
    if games.is_empty() {
        head.push(Line::from(vec![
            label,
            dim(if only {
                "none picked"
            } else {
                "none picked — the ones closest to dropping go first"
            }),
        ]));
    } else {
        let room_w = w.saturating_sub(LABEL + 1);
        for (i, l) in wrap_list(&games, room_w, 2).into_iter().enumerate() {
            let lead = if i == 0 {
                label.clone()
            } else {
                Span::raw(" ".repeat(LABEL + 1))
            };
            head.push(Line::from(vec![lead, Span::raw(l)]));
        }
    }
    if only {
        head.push(Line::from(vec![
            Span::styled(fit(" Others", LABEL + 1), theme::bold()),
            Span::styled("not farmed: \"only priority\" is on", theme::fg(BUSY)),
        ]));
    }
    let online = cx.prefs.appear_online;
    head.push(Line::from(vec![
        Span::styled(fit(" Friends", LABEL + 1), theme::bold()),
        Span::raw(if online {
            "see you online, and the games being played "
        } else {
            "see you offline, as you appear now "
        }),
        keycap("v"),
    ]));
    head.push(Line::default());
    head.extend(paragraph(
        "Farming runs while steamcards is open, so leave it running. Change any \
         of this later from the dashboard.",
        w,
    ));
    head.push(Line::default());
    head.push(selected_row(
        Line::from(vec![Span::raw(" Start farming →")]),
        w,
    ));
    if head.len() > room {
        head.retain(|l| width(&l.spans) > 0);
    }
    let mut card = Card::new("Ready to farm", head);
    card.keys = vec![
        ("enter", "start farming", 0),
        (
            "v",
            if online {
                "appear offline"
            } else {
                "appear online"
            },
            3,
        ),
        ("←", "back", 1),
        ("?", "help", 2),
        ("q", "quit", 1),
    ];
    card
}
