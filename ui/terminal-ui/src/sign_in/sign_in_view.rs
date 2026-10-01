//! The sign-in pop-up: the QR code to scan with the Steam app, and how the
//! sign-in went.

use std::time::Instant;

use account::LoginChallenge;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::Ctx,
    popup::{fit_height, modal, spinner_line},
    theme::{self, BAD, BUSY, GOOD},
    widgets::{hints, qr, wrap_text},
};

pub(crate) struct SignInView {
    pub(crate) challenge: Option<LoginChallenge>,
    /// `Ok(account name)` or `Err(reason)` once the sign-in ends.
    pub(crate) outcome: Option<Result<String, String>>,
    pub(crate) finished: Option<Instant>,
    /// Return to the account pop-up (rather than the dashboard) afterwards.
    pub(crate) from_account: bool,
}

pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &SignInView) {
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
