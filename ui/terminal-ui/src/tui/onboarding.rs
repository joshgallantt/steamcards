// First run, full screen and one step at a time (docs/design/ui.md, mockups
// o): a welcome, signing in with the Steam app, picking games, and a Start
// step that sizes up the job before it begins and says how values are read;
// the steps along the top, and each step's keys at the bottom. Signing out
// comes back to the sign-in step.
//
// A step is a card in the middle of the screen, 80 columns at most, its
// words 4 in from its border and, where there's room, a row clear above and
// below them. Where there isn't, the card gives up those rows first, then
// the Welcome its three-step list, which the steps along the top cover: the
// notice and "Press enter to agree" are never cut.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
};

use super::{
    CONTINUE_ROW, Ctx, format,
    text::{
        Fits, Hint, NB, Titles, first_fit, fit, glue, hints, join, key, panel, put, put_lines,
        spread, wrap,
    },
    theme,
};
use crate::viewmodel::{GameRow, Job, Step, Strip};

/// The card's width on roomy windows.
const CARD: usize = 80;
/// The Start step's labels: " Steam      ".
const LABEL: usize = 12;

/// A step: its title and a note in its top border, its lines (a list among
/// them that scrolls to keep its cursor in view), and its keys.
struct Card {
    title: &'static str,
    note: Option<String>,
    head: Vec<Line<'static>>,
    list: Vec<Line<'static>>,
    focus: usize,
    tail: Vec<Line<'static>>,
    keys: Vec<Hint>,
}

impl Card {
    fn new(title: &'static str, head: Vec<Line<'static>>, keys: Vec<Hint>) -> Self {
        Self {
            title,
            note: None,
            head,
            list: Vec::new(),
            focus: 0,
            tail: Vec::new(),
            keys,
        }
    }

    fn len(&self) -> usize {
        self.head.len() + self.list.len() + self.tail.len()
    }
}

pub(super) fn render(buf: &mut Buffer, cx: &Ctx<'_>, step: Step) {
    let area = *buf.area();
    let w = usize::from(area.width);
    put(buf, 0, 0, area.width, &steps(step, w));
    // The body: between the steps and the strip.
    let body = usize::from(area.height).saturating_sub(3);
    let card_w = CARD.min(w.saturating_sub(2));
    let inner = card_w.saturating_sub(8);
    let card = match step {
        Step::Welcome => welcome(inner, body),
        Step::SignIn => sign_in(cx, inner),
        Step::Games => games(cx, inner),
        Step::Start => start(cx, inner),
    };
    if let Ok(card) = card {
        draw(buf, &card, card_w, body);
        let keys = hints(&card.keys, w.saturating_sub(1)).unwrap_or_default();
        put(
            buf,
            0,
            area.bottom() - 1,
            area.width,
            &join([Line::from(" "), keys]),
        );
    }
    if let Strip::Flash { text, failed } = cx.app.strip(&cx.s) {
        let style = if failed {
            theme::strong(theme::BAD)
        } else {
            theme::bold()
        };
        let flash = Line::from(vec![
            Span::styled(" ▸ ", theme::fg(theme::ACCENT)),
            Span::styled(text, style),
        ]);
        if flash.width() <= w {
            put(buf, 0, area.bottom() - 2, area.width, &flash);
        }
    }
}

/// Draws `card`, `card_w` wide, in the middle of the `body` rows under the
/// steps: with a row clear above and below its words where there's room.
fn draw(buf: &mut Buffer, card: &Card, card_w: usize, body: usize) {
    let width = usize::from(buf.area().width);
    let padded = card.len() + 4 <= body;
    let pad = usize::from(padded);
    let h = (card.len() + 2 + 2 * pad).min(body);
    let x = (width - card_w) / 2;
    let y = 1 + (body - h) / 2;
    let area = Rect::new(x as u16, y as u16, card_w as u16, h as u16);
    let titles = Titles {
        top_left: Some(Line::styled(card.title, theme::brand())),
        top_right: card
            .note
            .clone()
            .map(|n| Line::styled(n, theme::dim()))
            .filter(|n| n.width() + card.title.len() + 8 <= card_w),
        ..Titles::default()
    };
    panel(buf, area, theme::modal_border(), &titles);
    let rows = h.saturating_sub(2 + 2 * pad);
    // The list gets what the head and tail leave, scrolled to its cursor.
    let list_rows = rows.saturating_sub(card.head.len() + card.tail.len());
    let from = if card.list.len() > list_rows {
        card.focus
            .saturating_sub(list_rows / 2)
            .min(card.list.len() - list_rows)
    } else {
        0
    };
    let mut lines = card.head.clone();
    lines.extend(card.list.iter().skip(from).take(list_rows).cloned());
    lines.extend(card.tail.iter().cloned());
    lines.truncate(rows);
    let inside = Rect::new(
        area.x + 4,
        area.y + 1 + pad as u16,
        area.width.saturating_sub(8),
        rows as u16,
    );
    put_lines(buf, inside, &lines);
}

fn name(s: Step) -> &'static str {
    match s {
        Step::Welcome => "Welcome",
        Step::SignIn => "Sign in",
        Step::Games => "Games",
        Step::Start => "Start",
    }
}

/// " steamcards    ✓ Welcome › 2 Sign in › 3 Games › 4 Start", or shorter.
fn steps(step: Step, w: usize) -> Line<'static> {
    let at = Step::ALL.iter().position(|s| *s == step).unwrap_or(0);
    let pill = || {
        vec![
            Span::styled(" steamcards ", theme::pill()),
            Span::raw("   "),
        ]
    };
    let mut full = pill();
    for (i, s) in Step::ALL.into_iter().enumerate() {
        if i > 0 {
            full.push(Span::styled(" › ", theme::dim()));
        }
        full.push(match i.cmp(&at) {
            std::cmp::Ordering::Less => {
                Span::styled(format!("✓ {}", name(s)), theme::fg(theme::GOOD))
            }
            std::cmp::Ordering::Equal => Span::styled(
                format!("{} {}", i + 1, name(s)),
                theme::strong(theme::ACCENT),
            ),
            std::cmp::Ordering::Greater => {
                Span::styled(format!("{} {}", i + 1, name(s)), theme::dim())
            }
        });
    }
    let mut short = pill();
    short.push(Span::styled(
        format!("Step {} of {} · ", at + 1, Step::ALL.len()),
        theme::dim(),
    ));
    short.push(Span::styled(name(step), theme::strong(theme::ACCENT)));
    first_fit([Line::from(full), Line::from(short.clone())], w)
        .unwrap_or_else(|_| Line::from(short))
}

/// "1  Sign in…": a numbered line.
fn numbered(n: &str, text: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{n}  "), theme::strong(theme::BUSY)),
        Span::raw(text.to_owned()),
    ])
}

// ── Welcome ──────────────────────────────────────────────────────────────────

/// The welcome, `w` wide, in `body` rows: what steamcards does, its three
/// steps where there's room, and the notice to agree to, whole.
fn welcome(w: usize, body: usize) -> Fits<Card> {
    let what = wrap(
        "steamcards plays your Steam games in the background, so their trading cards drop \
         while you get on with your day. Nothing is installed or launched: Steam is simply \
         told what's being played.",
        w,
        0,
    )?;
    let steps = vec![
        numbered("1", "Sign in with the Steam app on your phone"),
        numbered("2", "Pick the games you want cards from first"),
        numbered("3", "Start farming, and leave it running"),
    ];
    // Said before anything starts, so starting is agreeing to it.
    let mut notice = wrap(
        Line::styled(
            "steamcards is unofficial: Valve doesn't make or support it. Farming cards is at \
             your own risk.",
            theme::dim(),
        ),
        w,
        0,
    )?;
    notice.push(Line::default());
    notice.push(Line::from(vec![
        Span::raw("Press "),
        key("enter"),
        Span::raw(" to agree and begin."),
    ]));
    let mut head = what.clone();
    head.push(Line::default());
    // The steps go when the card wouldn't fit with them, borders and all.
    if what.len() + steps.len() + notice.len() + 4 <= body {
        head.extend(steps);
        head.push(Line::default());
    }
    head.extend(notice);
    Ok(Card::new(
        "Welcome to steamcards",
        head,
        vec![
            Hint::new("enter", "agree and begin", 0),
            Hint::new("?", "help", 2),
            Hint::new("q", "quit", 1),
        ],
    ))
}

// ── Sign in ──────────────────────────────────────────────────────────────────

/// The account, as the steps show it: "● alice", or that nobody is signed
/// in.
fn account(cx: &Ctx<'_>) -> Vec<Span<'static>> {
    match cx.s.account {
        Some(a) if a.expired => vec![Span::styled(
            format!("✕ {}: sign-in expired", a.name),
            theme::fg(theme::BAD),
        )],
        Some(a) => vec![
            Span::styled("●", theme::fg(theme::GOOD)),
            Span::raw(format!(
                " {}",
                if a.name.is_empty() {
                    "signed in"
                } else {
                    a.name.as_str()
                }
            )),
        ],
        None => vec![Span::styled("not signed in", theme::dim())],
    }
}

/// A row the cursor can be on, as the selection shows it.
fn selectable(line: Line<'static>, on: bool) -> Line<'static> {
    if !on {
        return line;
    }
    let mut line = line;
    for span in &mut line.spans {
        span.style = theme::selected();
    }
    line.style(theme::selected())
}

fn sign_in(cx: &Ctx<'_>, w: usize) -> Fits<Card> {
    let cursor = cx.app.setup.cursor;
    let signed_in = cx.s.account.is_some();
    let label = if signed_in {
        "sign in again"
    } else {
        "sign in"
    };
    let mut head = wrap(
        "Scan a QR code with the Steam app on your phone and approve it there. No password is \
         typed in here.",
        w,
        0,
    )?;
    head.push(Line::default());
    let mut left = vec![
        Span::raw(" "),
        Span::styled("Steam", theme::steam()),
        Span::raw("    "),
    ];
    left.extend(account(cx));
    let right = if cursor == 0 {
        Line::from(format!("{label} › "))
    } else {
        Line::default()
    };
    head.push(selectable(
        spread(Line::from(left), right, w, 1)?,
        cursor == 0,
    ));
    head.push(Line::default());
    let go = if cx.app.onboarding.can_continue() {
        Line::styled(" Continue →", theme::strong(theme::GOOD))
    } else {
        Line::from(vec![
            Span::styled(" Continue →", theme::dim()),
            Span::styled("  sign in first", theme::dim()),
        ])
    };
    head.push(selectable(fit(go, w)?, cursor == CONTINUE_ROW));
    let enter = if cursor == 0 { label } else { "continue" };
    Ok(Card::new(
        "Sign in to Steam",
        head,
        vec![
            Hint::new("↑↓", "select", 2),
            Hint::new("enter", enter, 0),
            Hint::new("→", "continue", 1),
            Hint::new("←", "back", 3),
            Hint::new("?", "help", 4),
            Hint::new("q", "quit", 1),
        ],
    ))
}

// ── Games ────────────────────────────────────────────────────────────────────

fn games(cx: &Ctx<'_>, w: usize) -> Fits<Card> {
    let app = cx.app;
    let rows = app.picks();
    let cursor = app.setup.cursor.min(rows.len().saturating_sub(1));
    let only = cx.s.prefs.only_priority;
    let mut head = wrap(
        if only {
            "The games you pick are farmed first, in the order you pick them."
        } else {
            "The games you pick are farmed first, in the order you pick them. Skip this and \
             steamcards farms the games closest to dropping first."
        },
        w,
        0,
    )?;
    head.push(Line::default());
    let list: Vec<Line<'static>> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| Ok(selectable(pick_row(r, w)?, i == cursor)))
        .collect::<Fits<_>>()?;
    // Where the library is at: under the games, or in their place.
    let mut tail = Vec::new();
    let status = library_status(cx, w)?;
    if !status.is_empty() {
        if !rows.is_empty() {
            tail.push(Line::default());
        }
        tail.extend(status);
    }
    if only {
        tail.push(Line::default());
        tail.extend(wrap(
            Line::styled(
                "\"Only priority\" is on: only the games you pick are farmed. Press o to turn it \
                 off.",
                theme::fg(theme::BUSY),
            ),
            w,
            0,
        )?);
    }
    let picked = cx.s.prefs.priority_games.len();
    let on_a_pick = rows.get(cursor).is_some_and(|r| r.rank.is_some());
    let mut card = Card::new(
        "Pick your games",
        head,
        vec![
            Hint::new("↑↓", "select", 2),
            Hint::new("space", if on_a_pick { "unpick" } else { "pick" }, 0),
            Hint::new("o", "only priority", 5),
            Hint::new("r", "read again", 6),
            Hint::new("enter", if picked == 0 { "skip" } else { "continue" }, 0),
            Hint::new("←", "back", 2),
            Hint::new("?", "help", 4),
            Hint::new("q", "quit", 1),
        ],
    );
    card.note = (picked > 0).then(|| format!("{picked} picked"));
    card.list = list;
    card.focus = cursor;
    card.tail = tail;
    Ok(card)
}

/// Reading, failed or empty: the library, when it's not simply there, in
/// lines of `w` columns; none when it is.
fn library_status(cx: &Ctx<'_>, w: usize) -> Fits<Vec<Line<'static>>> {
    let library = &cx.app.library;
    if library.is_loading() {
        return Ok(vec![Line::from(vec![
            Span::styled(cx.spinner, theme::fg(theme::BUSY)),
            Span::raw(" Reading your badges for games with cards left"),
        ])]);
    }
    let say = |text: String, style| wrap(Line::styled(text, style), w, 0);
    if let Some(e) = library.error() {
        return say(
            format!("Couldn't read your badges: {e}. Press r to try again, or skip this."),
            theme::fg(theme::BAD),
        );
    }
    match library.library() {
        None => say("Press r to read your badges.".to_owned(), theme::dim()),
        Some(l) if l.drops_left() == 0 => say(
            "No games with cards left to drop right now. Skip this: steamcards looks again \
             every few hours."
                .to_owned(),
            theme::dim(),
        ),
        Some(_) => Ok(Vec::new()),
    }
}

/// "#1  Portal 2                     3 cards to drop · 5.2h played".
fn pick_row(r: &GameRow, w: usize) -> Fits<Line<'static>> {
    let rank = match r.rank {
        Some(n) => Span::styled(
            format!("{:<4}", format!("#{n}")),
            theme::strong(theme::BUSY),
        ),
        None => Span::raw("    "),
    };
    let left = match r.drops.remaining {
        0 => "done".to_owned(),
        n => format!("{} to drop", format::cards(n as usize)),
    };
    let played = format::hours(r.hours);
    let room = w.saturating_sub(1 + 4 + 14);
    let facts = first_fit(
        [
            format!("  {left} · {played} played "),
            format!("  {left} · {played} "),
            format!("  {left} "),
        ]
        .map(|t| Line::styled(t, theme::dim())),
        room,
    )
    .unwrap_or_default();
    let name = super::text::shorten(&r.name, w.saturating_sub(5 + facts.width() + 1))?;
    spread(
        Line::from(vec![Span::raw(" "), rank, Span::raw(name)]),
        facts,
        w,
        1,
    )
}

// ── Start ────────────────────────────────────────────────────────────────────

/// A labelled row, " Steam      ● alice", and its lines after the first,
/// under its words.
fn labelled(label: &str, first: Line<'static>, rest: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let mut out = vec![join([
        Line::from(" "),
        fit(Line::styled(label.to_owned(), theme::heading()), LABEL - 1).unwrap_or_default(),
        first,
    ])];
    out.extend(
        rest.into_iter()
            .map(|l| join([Line::from(" ".repeat(LABEL)), l])),
    );
    out
}

/// Words wrapped to go beside a label, `w` wide.
fn beside(text: impl Into<Line<'static>>, w: usize) -> Fits<Vec<Line<'static>>> {
    wrap(text, w.saturating_sub(LABEL), 0)
}

/// Before farming starts: who's signed in, the games picked, how friends
/// see the games, the job sized up (the games and drops to farm and how long
/// that should take), how values are read, and the button to start.
fn start(cx: &Ctx<'_>, w: usize) -> Fits<Card> {
    let job = Job::build(&cx.s);
    let dim = theme::dim();
    let mut head = labelled("Steam", Line::from(account(cx)), Vec::new());
    let picked: Vec<String> = job
        .picked
        .iter()
        .enumerate()
        .map(|(i, n)| format!("#{} {n}", i + 1))
        .collect();
    let games = if picked.is_empty() {
        beside(
            Line::styled(
                if job.only_priority {
                    "none picked"
                } else {
                    "none picked: the ones closest to dropping go first"
                },
                dim,
            ),
            w,
        )?
    } else {
        beside(picked.join(", "), w)?
    };
    head.extend(split("Games", games));
    if job.only_priority {
        head.extend(split(
            "Others",
            beside(
                Line::styled(
                    "not farmed: \"only priority\" is on",
                    theme::fg(theme::BUSY),
                ),
                w,
            )?,
        ));
    }
    let friends = if job.appear_online {
        "see you online, and the games being played"
    } else {
        "see you offline, as you appear now"
    };
    // Two spaces before the key, which a wrap would make one.
    head.extend(split(
        "Friends",
        beside(
            Line::from(vec![Span::raw(format!("{friends}{NB} ")), key("v")]),
            w,
        )?,
    ));
    head.push(Line::default());
    let to_farm = match job.to_farm {
        Some((games, drops)) => format!("{} · {}", format::games(games), format::drops(drops)),
        None => "once your badges are read".to_owned(),
    };
    head.extend(split("To farm", beside(to_farm, w)?));
    if let Some((eta, hours)) = job.eta {
        let first = format!(
            "{}, if a card drops every 30 minutes",
            glue(&format::eta(eta))
        );
        let time = if hours.is_zero() {
            beside(format!("{first}. It learns as cards drop."), w)?
        } else {
            let mut t = beside(format!("{first},"), w)?;
            t.extend(beside(
                format!(
                    "plus {} building hours. It learns as cards drop.",
                    glue(&format!("≈ {}", format::estimate(hours)))
                ),
                w,
            )?);
            t
        };
        head.extend(split("Time", time));
    }
    let basis = match job.basis {
        market::Basis::List => "at list prices",
        market::Basis::Net => "after Steam's fees",
        market::Basis::Instant => "as sold now, to offers",
    };
    let mut values = beside(
        Line::from(vec![
            Span::raw(format!("{basis} ")),
            key("b"),
            Span::raw(", read from the Steam market"),
        ]),
        w,
    )?;
    values.extend(beside("a few games a minute, in the background", w)?);
    head.extend(split("Values", values));
    head.push(Line::default());
    head.extend(wrap(
        "Farming runs while steamcards is open, so leave it running. Change any of this later \
         from the dashboard.",
        w,
        0,
    )?);
    head.push(Line::default());
    head.push(selectable(fit(Line::from(" Start farming →"), w)?, true));
    let online = if job.appear_online {
        "appear offline"
    } else {
        "appear online"
    };
    Ok(Card::new(
        "Ready to farm",
        head,
        vec![
            Hint::new("enter", "start farming", 0),
            Hint::new("v", online, 3),
            Hint::new("b", "list/net/instant", 4),
            Hint::new("←", "back", 1),
            Hint::new("?", "help", 2),
            Hint::new("q", "quit", 1),
        ],
    ))
}

/// A label's rows: its first line beside it, the rest under the first.
fn split(label: &str, lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    let mut lines = lines.into_iter();
    let first = lines.next().unwrap_or_default();
    labelled(label, first, lines.collect())
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent};

    use super::super::{Overlay, Scroll, fixtures, golden};
    use crate::viewmodel::Step;

    #[tokio::test]
    async fn every_step_fits_at_the_edges_of_each_size_class() {
        // Back from Start, a step at a time, each drawn at the edges of
        // every size class, alone and under help.
        let mut app = fixtures::for_mockup("onboarding, ready to farm").unwrap();
        for step in [Step::Start, Step::Games, Step::SignIn, Step::Welcome] {
            assert_eq!(app.onboarding.step(), Some(step));
            for w in [60, 71, 72, 99, 100, 120, 200, 240] {
                for h in [16, 19, 20, 25, 26, 40, 70] {
                    for help in [false, true] {
                        app.overlay = help.then(|| Overlay::Help(Scroll::default()));
                        let screen = golden::screen_text(&golden::render(&mut app, w, h));
                        assert_eq!(screen.len(), usize::from(h), "{step:?} at {w}×{h}");
                    }
                }
            }
            app.overlay = None;
            app.key(KeyEvent::from(KeyCode::Left));
        }
    }

    #[tokio::test]
    async fn the_start_step_is_the_spec_s() {
        let title = "onboarding, ready to farm";
        golden::check(title, &mut fixtures::for_mockup(title).unwrap());
    }

    #[tokio::test]
    async fn before_farming_starts_the_job_is_sized_up_from_the_library() {
        // As it is before the farmer has said anything: the library read
        // for browsing, and no time to finish yet.
        let mut app = fixtures::for_mockup("onboarding, ready to farm").unwrap();
        app.status = None;
        app.forecast = None;
        app.library.refresh();
        for _ in 0..100 {
            app.library.poll();
            if app.library.library().is_some() {
                break;
            }
            tokio::task::yield_now().await;
        }
        app.update_forecast();
        let f = app.forecast.clone().expect("a first estimate");
        assert!(f.assumed && f.band.is_none(), "30 minutes a drop, for now");
        assert_eq!(
            f.per_game.len(),
            62,
            "the games the farmer will farm, in its order"
        );
        let screen = golden::screen_text(&golden::render(&mut app, 100, 28)).join("\n");
        assert!(
            screen.contains("To farm    62 games · 252 drops"),
            "{screen}"
        );
        assert!(screen.contains("if a card drops every 30 minutes"));

        // Picking a game changes the order, and the estimate with it.
        app.games.toggle(crate::viewmodel::fixtures::STRAY).unwrap();
        let before = f.made_at;
        app.clock = super::super::Clock::Fixed {
            now: before + chrono::TimeDelta::seconds(5),
            zone: crate::viewmodel::fixtures::zone(),
        };
        app.update_forecast();
        assert_ne!(app.forecast.map(|f| f.made_at), Some(before));
    }

    #[tokio::test]
    async fn the_start_says_how_values_are_read_on_each_basis() {
        for (basis, words) in [
            (
                market::Basis::List,
                "at list prices [b], read from the Steam market",
            ),
            (
                market::Basis::Net,
                "after Steam's fees [b], read from the Steam market",
            ),
            (
                market::Basis::Instant,
                "as sold now, to offers [b], read from the",
            ),
        ] {
            let mut data = crate::viewmodel::fixtures::first_minutes();
            data.run = crate::viewmodel::Run::Stopped;
            data.basis = basis;
            let mut app = fixtures::app(data);
            app.onboarding.signed_out();
            app.onboarding_next();
            app.onboarding_next();
            let values = golden::screen_text(&golden::render(&mut app, 100, 28))
                .into_iter()
                .find(|row| row.contains("Values"))
                .unwrap_or_default();
            assert!(values.contains(words), "{values}");
        }
    }

    #[tokio::test]
    async fn the_welcome_is_the_spec_s_at_60_by_16() {
        let title = "onboarding, welcome (XS)";
        golden::check(title, &mut fixtures::for_mockup(title).unwrap());
    }

    #[tokio::test]
    async fn the_welcome_lists_its_steps_where_there_is_room() {
        let mut app = fixtures::for_mockup("onboarding, welcome (XS)").unwrap();
        let screen = golden::screen_text(&golden::render(&mut app, 100, 30)).join("\n");
        assert!(screen.contains("1  Sign in with the Steam app on your phone"));
        assert!(screen.contains("Press [enter] to agree and begin."));
    }
}
