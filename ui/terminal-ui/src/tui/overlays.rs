// Pop-ups drawn over the dashboard (sign-in and help also over onboarding):
// the account, signing in, games, the full log, a game's details (narrow
// windows), help, and quit confirmation.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

use super::{
    Ctx, GamesView, LogView, LoginView, Overlay, dashboard, onboarding,
    theme::{self, ACCENT, BAD, BUSY, GOOD, SELECT},
    widgets::{
        centered, fit, flash_line, hints, keycap, qr, scroll, selected_row, spread, truncate,
        wrap_text,
    },
};

/// Draws the open pop-up, if any. For the log, returns its (offset, max) so the
/// app can remember where it's scrolled to.
pub(super) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) -> Option<(usize, usize)> {
    let overlay = cx.app.overlay.as_ref()?;
    // Fade what's underneath so the pop-up stands out.
    f.buffer_mut()
        .set_style(area, Style::new().add_modifier(Modifier::DIM));
    let mut log_scroll = None;
    match overlay {
        Overlay::Help => help(f, area, cx),
        Overlay::Account { confirm } => account(f, area, cx, *confirm),
        Overlay::Login(v) => login(f, area, cx, v),
        Overlay::Games(v) => games(f, area, cx, v),
        Overlay::Log(v) => log_scroll = Some(log(f, area, cx, v)),
        Overlay::Detail => detail(f, area, cx),
        Overlay::ConfirmQuit => confirm_quit(f, area),
    }
    // A pop-up can cover the strip where messages go, so they go on the
    // bottom row, over the faded footer, which no pop-up covers: what a key
    // just did, or that it didn't stick, is always on screen. (Help, which
    // changes nothing, can fill a short window.)
    let flash = cx
        .app
        .flash
        .as_ref()
        .filter(|_| !matches!(overlay, Overlay::Help));
    if let Some((msg, _)) = flash {
        let row = Rect {
            y: area.bottom().saturating_sub(1),
            height: area.height.min(1),
            ..area
        };
        f.render_widget(Clear, row);
        f.render_widget(Paragraph::new(flash_line(msg)), row);
    }
    log_scroll
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

/// A line with a space either side, to sit in a border.
fn pad(mut l: Line<'static>) -> Line<'static> {
    l.spans.insert(0, Span::raw(" "));
    l.spans.push(Span::raw(" "));
    l
}

/// A centred pop-up frame `w`×`h` (borders included) with key hints in its
/// bottom edge. Returns the area inside it.
fn modal(f: &mut Frame<'_>, area: Rect, w: u16, h: u16, title: &str, keys: Line<'static>) -> Rect {
    modal_in(f, area, w, h, title, keys, ACCENT)
}

/// `modal` with a chosen border colour (the details pop-up uses the selection
/// colour, like the panel it stands in for).
fn modal_in(
    f: &mut Frame<'_>,
    area: Rect,
    w: u16,
    h: u16,
    title: &str,
    keys: Line<'static>,
    color: Color,
) -> Rect {
    let r = centered(area, w, h);
    f.render_widget(Clear, r);
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::fg(color))
        .title(Line::styled(format!(" {title} "), theme::strong(color)))
        .title_bottom(pad(keys).right_aligned())
        .padding(Padding::new(2, 2, 1, 0));
    let inner = block.inner(r);
    f.render_widget(block, r);
    inner
}

/// Height for a pop-up holding `lines` rows: borders, top padding, and one
/// blank row above the bottom edge.
fn fit_height(lines: usize) -> u16 {
    lines as u16 + 4
}

fn spinner_line(cx: &Ctx<'_>, text: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{} ", cx.spinner()), theme::fg(BUSY)),
        dim(text.to_owned()),
    ])
}

/// "◉ label [k]": an option that's on or off, with the key that flips it.
fn toggle(on: bool, label: &str, key: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(
                "{} ",
                if on {
                    theme::RADIO_ON
                } else {
                    theme::RADIO_OFF
                }
            ),
            if on {
                theme::strong(GOOD)
            } else {
                theme::dim()
            },
        ),
        Span::raw(format!("{label} ")),
        keycap(key),
    ])
}

// ── Account ──────────────────────────────────────────────────────────────────

fn account(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, confirm: bool) {
    const W: u16 = 66;
    let inner_w = (W - 6) as usize;
    if confirm {
        let who = cx
            .account
            .map(|a| a.name.clone())
            .filter(|n| !n.is_empty())
            .map_or_else(String::new, |n| format!(" ({n})"));
        let lines = vec![
            Line::from(vec![
                Span::styled("Sign out of Steam", theme::strong(BAD)),
                Span::raw(format!("{who}?")),
            ]),
            Line::default(),
            Line::styled(
                "Farming stops, and Steam ends this sign-in. Signing in again",
                theme::dim(),
            ),
            Line::styled("takes a new QR scan.", theme::dim()),
        ];
        let keys = hints(&[("y", "sign out", 0), ("n", "keep", 0)], inner_w);
        let inner = modal(f, area, W, fit_height(lines.len()), "Account", keys);
        f.render_widget(Paragraph::new(lines), inner);
        return;
    }
    let mut left = vec![
        Span::raw(" "),
        Span::styled(fit("Steam", 9), theme::steam()),
    ];
    left.extend(dashboard::account_badge(cx));
    let mut right = match cx.account {
        Some(a) if a.expired => vec![Span::styled("sign in again", theme::fg(BAD))],
        Some(_) if cx.app.farming.is_running() => vec![Span::styled("farming", theme::fg(GOOD))],
        Some(_) => vec![Span::styled("paused", theme::fg(BUSY))],
        None => Vec::new(),
    };
    right.push(Span::raw(" "));
    let mut lines = vec![
        selected_row(spread(left, right, inner_w), inner_w),
        Line::default(),
    ];
    lines.extend(
        wrap_text(
            "Signs in with the Steam app on your phone: scan a QR code and approve.",
            inner_w,
        )
        .into_iter()
        .map(|l| Line::styled(l, theme::dim())),
    );
    lines.push(Line::default());
    lines.push(toggle(
        !cx.prefs.appear_online,
        "Appear offline while farming",
        "v",
    ));
    lines.extend(
        wrap_text(
            if cx.prefs.appear_online {
                "Friends see you online, and every game being played."
            } else {
                "Friends don't see the games being played. Steam counts them just the same."
            },
            inner_w.saturating_sub(2),
        )
        .into_iter()
        .map(|l| Line::styled(format!("  {l}"), theme::dim())),
    );

    let enter = if cx.signed_in() {
        "sign in again"
    } else {
        "sign in"
    };
    let mut keys = vec![("enter", enter, 0), ("v", "offline/online", 2)];
    if cx.signed_in() {
        keys.push(("d", "sign out", 1));
    }
    keys.push(("esc", "close", 0));
    let keys = hints(&keys, inner_w);
    let inner = modal(f, area, W, fit_height(lines.len()), "Account", keys);
    f.render_widget(Paragraph::new(lines), inner);
}

// ── Sign-in ──────────────────────────────────────────────────────────────────

fn login(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &LoginView) {
    let title = "Sign in to Steam";
    match (&v.outcome, &v.challenge) {
        (Some(Ok(who)), _) => {
            let who = if who.is_empty() {
                String::new()
            } else {
                format!(" as {who}")
            };
            let next = if cx.app.onboarding.is_active() {
                "Next up: picking the games you want cards from first."
            } else if cx.app.farming.is_running() {
                "Farming carries on with it straight away."
            } else {
                "Press p on the dashboard to start farming."
            };
            let lines = vec![
                Line::from(vec![
                    Span::styled(format!("{} ", theme::DONE), theme::fg(GOOD)),
                    Span::styled("Signed in", theme::strong(GOOD)),
                    Span::raw(who),
                ]),
                Line::default(),
                Line::styled(next, theme::dim()),
            ];
            let inner = modal(
                f,
                area,
                62,
                fit_height(lines.len()),
                title,
                hints(&[("enter", "done", 0)], 50),
            );
            f.render_widget(Paragraph::new(lines), inner);
        }
        (Some(Err(e)), _) => {
            const W: u16 = 64;
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(format!("{} ", theme::FAILED), theme::fg(BAD)),
                    Span::styled("Couldn't sign in", theme::strong(BAD)),
                ]),
                Line::default(),
            ];
            lines.extend(
                wrap_text(e, (W - 6) as usize)
                    .into_iter()
                    .map(|l| Line::styled(l, theme::dim())),
            );
            let keys = hints(&[("r", "try again", 0), ("esc", "close", 0)], 50);
            let inner = modal(f, area, W, fit_height(lines.len()), title, keys);
            f.render_widget(Paragraph::new(lines), inner);
        }
        (None, Some(c)) => qr_code(f, area, cx, title, &c.url, c.scanned),
        (None, None) => {
            let lines = vec![spinner_line(cx, "Asking Steam for a QR code…")];
            let inner = modal(
                f,
                area,
                48,
                fit_height(lines.len()),
                title,
                hints(&[("esc", "cancel", 0)], 40),
            );
            f.render_widget(Paragraph::new(lines), inner);
        }
    }
}

fn qr_code(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, title: &str, url: &str, scanned: bool) {
    const TEXT_W: usize = 46;
    /// The text is never taller than this, so the QR code sets the height.
    const TEXT_H: usize = 9;
    let step = |n: &'static str, verb: &'static str, what: &'static str, done: bool| {
        Line::from(vec![
            if done {
                Span::styled(format!("{}  ", theme::DONE), theme::strong(GOOD))
            } else {
                Span::styled(format!("{n}  "), theme::strong(BUSY))
            },
            Span::raw(format!("{verb:<8}")),
            Span::raw(what),
        ])
    };

    let code = qr(url);
    let qr_w = code.first().map_or(0, |l| l.chars().count());
    let w = (qr_w + 3 + TEXT_W + 6) as u16;
    let h = (code.len().max(TEXT_H) + 3) as u16;
    let with_qr = !code.is_empty() && w + 2 <= area.width && h + 2 <= area.height;

    let mut text = vec![
        Line::styled("Sign in with the Steam app", theme::bold()),
        Line::default(),
        step("1", "Scan", "the code in the Steam app", scanned),
        step("2", "Approve", "the sign-in on your phone", false),
        Line::default(),
    ];
    if with_qr {
        text.push(Line::styled(
            "The scanner is in the app's Steam Guard tab.",
            theme::dim(),
        ));
        text.push(Line::styled("No password is typed in here.", theme::dim()));
    } else {
        text.push(Line::styled(
            "Make the window bigger to show the QR code.",
            theme::fg(BUSY),
        ));
    }
    text.push(Line::default());
    text.push(spinner_line(
        cx,
        if scanned {
            "Scanned — waiting for you to approve…"
        } else {
            "Waiting for the Steam app…"
        },
    ));
    let keys = hints(&[("esc", "cancel", 0)], 40);

    if !with_qr {
        let inner = modal(
            f,
            area,
            (TEXT_W + 6) as u16,
            fit_height(text.len()),
            title,
            keys,
        );
        f.render_widget(Paragraph::new(text), inner);
        return;
    }
    let inner = modal(f, area, w, h, title, keys);
    let [left, _, right] = Layout::horizontal([
        Constraint::Length(qr_w as u16),
        Constraint::Length(3),
        Constraint::Min(10),
    ])
    .areas(inner);
    // True black on true white whatever the terminal theme (named colours are
    // softened by many themes), so phone cameras read it reliably.
    let ink = Style::new()
        .fg(Color::Rgb(0, 0, 0))
        .bg(Color::Rgb(255, 255, 255));
    f.render_widget(
        Paragraph::new(
            code.into_iter()
                .map(|l| Line::styled(l, ink))
                .collect::<Vec<_>>(),
        ),
        left,
    );
    let top = (left.height as usize).saturating_sub(text.len()) / 2;
    let mut lines = vec![Line::default(); top];
    lines.extend(text);
    f.render_widget(Paragraph::new(lines), right);
}

// ── Games ────────────────────────────────────────────────────────────────────

fn games(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &GamesView) {
    const W: u16 = 76;
    let inner_w = (W - 6) as usize;
    let library = cx.app.known_library();
    let rows = cx.app.games.rows(library.games());
    let cursor = v.cursor.min(rows.len().saturating_sub(1));

    let mut lines = vec![
        Line::styled(
            "Your priority games are farmed first, in this order. The rest follow,",
            theme::dim(),
        ),
        Line::styled("the ones closest to dropping first.", theme::dim()),
        Line::default(),
    ];
    let mut cursor_line = 0;
    let first_other = rows.iter().position(|r| r.rank.is_none());
    for (i, r) in rows.iter().enumerate() {
        if i == 0 && r.rank.is_some() {
            lines.push(Line::styled("PRIORITY", theme::strong(BUSY)));
        }
        if Some(i) == first_other {
            if i > 0 {
                lines.push(Line::default());
            }
            lines.push(Line::styled(
                "OTHER GAMES WITH CARDS TO DROP",
                theme::bold(),
            ));
        }
        if i == cursor {
            cursor_line = lines.len();
        }
        let badge = match r.rank {
            Some(n) => Span::styled(fit(&format!("#{n}"), 4), theme::strong(BUSY)),
            None => Span::raw("    "),
        };
        let left = vec![Span::raw(" "), badge, Span::raw(r.name.clone())];
        let right = onboarding::game_facts(r, inner_w.saturating_sub(1 + 4 + 16));
        let row = spread(left, right, inner_w);
        lines.push(if i == cursor {
            selected_row(row, inner_w)
        } else {
            row
        });
    }
    let status = onboarding::library_status(cx, inner_w);
    if !status.is_empty() {
        if !rows.is_empty() {
            lines.push(Line::default());
        }
        lines.extend(status);
    }

    lines.push(Line::default());
    lines.push(toggle(
        cx.prefs.only_priority,
        "Only farm priority games",
        "o",
    ));

    let picked = rows.get(cursor).is_some_and(|r| r.rank.is_some());
    let keys = hints(
        &[
            ("↑↓", "select", 2),
            ("space", if picked { "unpick" } else { "pick" }, 0),
            ("1-9", "rank", 1),
            ("o", "only priority", 4),
            ("r", "read again", 3),
            ("esc", "close", 0),
        ],
        inner_w,
    );
    let h = fit_height(lines.len()).min(area.height.saturating_sub(2));
    let inner = modal(f, area, W, h, "Games", keys);
    let off = scroll(cursor_line, lines.len(), inner.height as usize);
    f.render_widget(Paragraph::new(lines).scroll((off as u16, 0)), inner);
}

// ── Log ──────────────────────────────────────────────────────────────────────

fn log(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &LogView) -> (usize, usize) {
    let r = Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    };
    f.render_widget(Clear, r);
    let n = cx.app.log.len();
    let keys = hints(
        &[
            ("↑↓", "scroll", 1),
            ("PgUp PgDn", "page", 2),
            ("home end", "jump", 2),
            ("esc", "close", 0),
        ],
        r.width.saturating_sub(4) as usize,
    );
    let mut block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::modal_border())
        .title(Line::styled(" Log ", theme::brand()))
        .title_bottom(pad(keys).right_aligned())
        .padding(Padding::horizontal(1));
    let inner = block.inner(r);
    let h = inner.height as usize;
    let max = n.saturating_sub(h);
    let off = if v.follow { max } else { v.offset.min(max) };
    let state = if v.follow || off == max {
        "following"
    } else {
        "scrolled back"
    };
    block = block.title_top(Line::from(dim(format!(" {n} events · {state} "))).right_aligned());
    f.render_widget(block, r);

    if n == 0 {
        f.render_widget(
            Paragraph::new(Line::styled("Nothing yet.", theme::dim())),
            inner,
        );
    } else {
        let lines: Vec<Line<'_>> = cx.app.log[off..(off + h).min(n)]
            .iter()
            .map(|e| dashboard::log_line(e, false))
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
    (off, max)
}

// ── Details (narrow windows) ─────────────────────────────────────────────────

fn detail(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    let Some(e) = cx.selected() else {
        return;
    };
    let w = area.width.saturating_sub(4).min(78);
    let inner_w = w.saturating_sub(6) as usize;
    let mut lines = dashboard::detail_info(e, cx, inner_w);
    lines.push(Line::default());
    lines.extend(dashboard::tier_controls(e, inner_w));
    let keys = hints(
        &[("↑↓", "previous / next game", 1), ("esc", "close", 0)],
        inner_w,
    );
    let h = fit_height(lines.len()).min(area.height.saturating_sub(2));
    let name = truncate(&e.game.name, inner_w.saturating_sub(2));
    let inner = modal_in(f, area, w, h, &name, keys, SELECT);
    f.render_widget(Paragraph::new(lines), inner);
}

// ── Help ─────────────────────────────────────────────────────────────────────

fn help(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
    /// Wide enough for two columns: 57 and 55 characters, 2 apart.
    const TWO_COLUMNS: u16 = 120;
    let num = |n: &'static str, text: &'static str| {
        Line::from(vec![
            Span::styled(format!(" {n}  "), theme::strong(BUSY)),
            Span::raw(text),
        ])
    };
    // Keys as keycaps; several keys on one row are separated by spaces.
    let row = |keys: &'static str, text: &'static str| {
        let mut spans = vec![Span::raw(" ")];
        let mut used = 1;
        for (i, k) in keys.split(' ').enumerate() {
            if i > 0 {
                spans.push(Span::raw(" "));
                used += 1;
            }
            spans.push(keycap(k));
            used += k.chars().count() + 2;
        }
        spans.push(Span::raw(" ".repeat(18usize.saturating_sub(used))));
        spans.push(Span::raw(text));
        Line::from(spans)
    };
    let head = |t: &'static str| Line::styled(t, theme::heading());

    let farmed = vec![
        head("What gets farmed"),
        num("1", "Your priority games (1–9), #1 first"),
        num("2", "Games whose cards can drop now, fewest left first"),
        num("3", "Games still building hours, most played first"),
        Line::styled("    Skipped games (x) are never farmed.", theme::dim()),
        Line::default(),
        Line::styled(
            " Cards drop for one game at a time, once it has 3 hours",
            theme::dim(),
        ),
        Line::styled(
            " on record. Games short of that play together, up to 32,",
            theme::dim(),
        ),
        Line::styled(" to build hours.", theme::dim()),
    ];
    // Each column's sections, top to bottom; one column shows them all.
    let (left, right) = if cx.app.onboarding.is_active() {
        // Only setting up's own keys work until farming starts.
        let setting_up = vec![
            head("Setting up"),
            row("↑↓", "choose"),
            row("enter", "go on, or sign in"),
            row("space", "pick or unpick a game"),
            row("→ ←", "next or previous step"),
            row("o", "only priority on or off"),
            row("v", "appear offline or online"),
            row("r", "read your badges again"),
            row("?", "this help"),
            row("q", "quit"),
        ];
        (vec![farmed], vec![setting_up])
    } else {
        let queue = vec![
            head("Queue"),
            row("↑↓", "choose a game (PgUp/PgDn jump)"),
            row("1-9", "rank the selected game"),
            row("0", "back to indifferent"),
            row("x", "skip it"),
            row("o", "open its card page"),
            row("c", "show or hide finished games"),
            row("enter", "details (on narrow windows)"),
        ];
        let everywhere = vec![
            head("Everywhere"),
            row("a", "account: sign in or out, appear offline"),
            row("g", "games: pick priority games"),
            row("l", "the full log"),
            row("p", "pause or carry on farming"),
            row("?", "this help"),
            row("q", "quit"),
        ];
        (vec![farmed, queue, legend(head)], vec![everywhere])
    };
    let stack = |sections: Vec<Vec<Line<'static>>>| -> Vec<Line<'static>> {
        let mut out = Vec::new();
        for s in sections {
            if !out.is_empty() {
                out.push(Line::default());
            }
            out.extend(s);
        }
        out
    };
    let (left, right) = (stack(left), stack(right));

    let keys = hints(&[("esc", "close", 0)], 20);
    let tall = fit_height(left.len() + 1 + right.len());
    if tall + 2 <= area.height || area.width < TWO_COLUMNS {
        let stacked = stack(vec![left, right]);
        let inner = modal(f, area, 66, tall, "Help", keys);
        f.render_widget(Paragraph::new(stacked), inner);
        return;
    }
    // Short window: two columns side by side.
    let inner = modal(
        f,
        area,
        TWO_COLUMNS,
        fit_height(left.len().max(right.len())),
        "Help",
        keys,
    );
    let [l, _, r] = Layout::horizontal([
        Constraint::Length(57),
        Constraint::Length(2),
        Constraint::Min(10),
    ])
    .areas(inner);
    f.render_widget(Paragraph::new(left), l);
    f.render_widget(Paragraph::new(right), r);
}

/// What the dashboard's symbols mean.
fn legend(head: impl Fn(&'static str) -> Line<'static>) -> Vec<Line<'static>> {
    let symbol =
        |glyph: Span<'static>, what: &'static str| [glyph, Span::raw(format!(" {what:<12}"))];
    vec![
        head("Symbols"),
        Line::from(
            [
                symbol(
                    Span::styled(format!(" {}", theme::FARMING), theme::fg(GOOD)),
                    "farming",
                ),
                symbol(Span::styled(" #1", theme::strong(BUSY)), "priority"),
                symbol(
                    Span::styled(format!(" {}", theme::DONE), theme::fg(GOOD)),
                    "done",
                ),
            ]
            .concat(),
        ),
        Line::from(
            [
                symbol(
                    Span::styled(format!(" {}", theme::HOURS), theme::fg(GOOD)),
                    "hours",
                ),
                symbol(
                    Span::styled(format!("  {}", theme::SKIPPED), theme::fg(BAD)),
                    "skipped",
                ),
                symbol(Span::styled("  ", theme::selected()), "selected"),
            ]
            .concat(),
        ),
    ]
}

// ── Quit ─────────────────────────────────────────────────────────────────────

fn confirm_quit(f: &mut Frame<'_>, area: Rect) {
    let lines = vec![
        Line::styled("Stop farming and quit?", theme::bold()),
        Line::default(),
        Line::styled(
            "Cards that dropped stay in your Steam inventory.",
            theme::dim(),
        ),
    ];
    let keys = hints(&[("y", "quit", 0), ("n", "keep farming", 0)], 50);
    let inner = modal(f, area, 60, fit_height(lines.len()), "Quit", keys);
    f.render_widget(Paragraph::new(lines), inner);
}
