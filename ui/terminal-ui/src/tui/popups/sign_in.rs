// Signing in with a QR code (docs/design/ui.md, mockup m): as today. The code
// beside how to scan it and what's happening, or above it when the window is
// narrower; the words alone when there isn't room for the code. Then whether
// it worked.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
};

use super::{
    super::{
        Ctx,
        text::{Fits, Hint, fit, join, wrap},
        theme,
    },
    LoginView, Shown,
};

/// Between the code and the words beside it.
const GAP: usize = 5;
/// The words beside the code need this much.
const WORDS: usize = 44;

pub(super) fn shown(cx: &Ctx<'_>, area: Rect, v: &LoginView) -> Fits<Shown> {
    let w = usize::from(area.width).saturating_sub(6);
    let room = usize::from(area.height).saturating_sub(2);
    let busy = theme::fg(theme::BUSY);
    let title = "Sign in to Steam";
    let (lines, keys): (Vec<Line<'static>>, Vec<Hint>) = match (&v.outcome, &v.challenge) {
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
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("✓ ", theme::fg(theme::GOOD)),
                    Span::styled("Signed in", theme::strong(theme::GOOD)),
                    Span::raw(who),
                ]),
                Line::default(),
            ];
            lines.extend(wrap(Line::styled(next, theme::dim()), w, 0)?);
            (lines, vec![Hint::new("enter", "done", 0)])
        }
        (Some(Err(e)), _) => {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("✕ ", theme::fg(theme::BAD)),
                    Span::styled("Couldn't sign in", theme::strong(theme::BAD)),
                ]),
                Line::default(),
            ];
            lines.extend(wrap(Line::styled(e.clone(), theme::dim()), w, 0)?);
            (
                lines,
                vec![Hint::new("r", "try again", 0), Hint::new("esc", "close", 0)],
            )
        }
        (None, Some(c)) => (
            with_code(&qr(&c.url), c.scanned, cx.spinner, w, room),
            vec![Hint::new("esc", "cancel", 0)],
        ),
        (None, None) => (
            vec![Line::from(vec![
                Span::styled(cx.spinner, busy),
                Span::styled(" Asking Steam for a QR code", theme::dim()),
            ])],
            vec![Hint::new("esc", "cancel", 0)],
        ),
    };
    // Set in the middle of the pop-up, top to bottom.
    let mut centred = vec![Line::default(); room.saturating_sub(lines.len()) / 2];
    centred.extend(lines);
    Ok(Shown::new(title, centred, &keys))
}

/// The code with the words beside it when both fit, or above them, or the
/// words alone, saying the window needs to be bigger.
fn with_code(
    code: &[String],
    scanned: bool,
    spinner: &'static str,
    w: usize,
    room: usize,
) -> Vec<Line<'static>> {
    let busy = theme::fg(theme::BUSY);
    let good = theme::fg(theme::GOOD);
    let step = |n: Span<'static>, verb: &str, what: &str| {
        join([
            Line::from(n),
            Line::from("  "),
            fit(Line::from(verb.to_owned()), 9).unwrap_or_default(),
            Line::from(what.to_owned()),
        ])
    };
    let scan = if scanned {
        Span::styled("✓", good)
    } else {
        Span::styled("1", theme::strong(theme::BUSY))
    };
    let waiting = if scanned {
        " Scanned — approve it in the app"
    } else {
        " Waiting for the Steam app"
    };
    let words = vec![
        Line::styled("Sign in with the Steam app", theme::bold()),
        Line::default(),
        step(scan, "Scan", "the code in the Steam app"),
        step(
            Span::styled("2", theme::strong(theme::BUSY)),
            "Approve",
            "the sign-in on your phone",
        ),
        Line::default(),
        Line::styled("The scanner is in the app's Steam Guard tab.", theme::dim()),
        Line::styled("No password is typed in here.", theme::dim()),
        Line::default(),
        Line::from(vec![
            Span::styled(spinner, busy),
            Span::styled(waiting, theme::dim()),
        ]),
    ];
    let cw = code.first().map_or(0, |l| l.chars().count());
    let ink = |l: &String| Line::styled(l.clone(), ink());
    if !code.is_empty() && w >= cw + GAP + WORDS && room >= code.len() {
        let top = code.len().saturating_sub(words.len()) / 2;
        return code
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let beside = i
                    .checked_sub(top)
                    .and_then(|j| words.get(j))
                    .cloned()
                    .unwrap_or_default();
                join([ink(l), Line::from(" ".repeat(GAP)), beside])
            })
            .collect();
    }
    if !code.is_empty() && w >= WORDS.max(cw) && room > code.len() + words.len() {
        let mut out: Vec<Line<'static>> = code.iter().map(ink).collect();
        out.push(Line::default());
        out.extend(words);
        return out;
    }
    let mut out: Vec<Line<'static>> = words[..5].to_vec();
    out.push(Line::styled(
        "Make the window bigger to show the QR code.",
        busy,
    ));
    out.extend(words[7..].iter().cloned());
    out
}

/// True black on true white, whatever the terminal's theme (named colours
/// are softened by many themes), so phone cameras read it reliably: the
/// one exception to the named colours.
fn ink() -> Style {
    Style::new()
        .fg(Color::Rgb(0, 0, 0))
        .bg(Color::Rgb(255, 255, 255))
}

/// Half-block QR code rows, with a two-module quiet zone.
fn qr(url: &str) -> Vec<String> {
    use qrcode::{Color as QrColor, EcLevel, QrCode};

    let Ok(code) = QrCode::with_error_correction_level(url, EcLevel::L) else {
        return Vec::new();
    };
    let n = code.width();
    let dark: Vec<bool> = code
        .to_colors()
        .into_iter()
        .map(|c| c == QrColor::Dark)
        .collect();
    const PAD: usize = 2;
    let size = n + PAD * 2;
    let at = |r: usize, c: usize| {
        (PAD..n + PAD).contains(&r)
            && (PAD..n + PAD).contains(&c)
            && dark[(r - PAD) * n + (c - PAD)]
    };
    (0..size)
        .step_by(2)
        .map(|r| {
            (0..size)
                .map(|c| match (at(r, c), at(r + 1, c)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}
