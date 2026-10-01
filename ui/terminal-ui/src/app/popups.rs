//! The pop-ups, drawn over the dashboard (sign-in and help also over
//! onboarding): the one open, over what's underneath, faded, with what a key
//! just did kept in view. Each feature draws its own; quitting is the
//! app's.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Clear, Paragraph},
};

use crate::{
    account::account_view,
    app::{Ctx, Overlay},
    dashboard::{detail_view, log_view},
    games::games_view,
    help::help_view,
    popup::{fit_height, modal},
    sign_in::sign_in_view,
    theme,
    widgets::{flash_line, hints},
};

/// Draws the open pop-up, if any. For the log and the details, returns their
/// (offset, max) so the app can remember where they're scrolled to.
pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) -> Option<(usize, usize)> {
    let overlay = cx.app.overlay.as_ref()?;
    // What's underneath fades, and all but its top line and bottom two goes,
    // so no half-hidden words show at the pop-up's edges.
    f.buffer_mut()
        .set_style(area, Style::new().add_modifier(Modifier::DIM));
    let middle = Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(3),
        ..area
    };
    f.render_widget(Clear, middle);
    // Pop-ups stay in the middle: the top line and the bottom two keep what's
    // happening and what a key just did in view.
    let area = middle;
    let mut log_scroll = None;
    match overlay {
        Overlay::Help => help_view::render(f, area, cx),
        Overlay::Account { confirm } => account_view::render(f, area, cx, *confirm),
        Overlay::SignIn(v) => sign_in_view::render(f, area, cx, v),
        Overlay::Games(v) => games_view::render(f, area, cx, v),
        Overlay::Log(v) => log_scroll = Some(log_view::render(f, area, cx, v)),
        Overlay::Detail { scroll } => log_scroll = detail_view::render(f, area, cx, *scroll),
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
            y: middle.bottom() + 1,
            height: 1,
            ..middle
        };
        f.render_widget(Clear, row);
        f.render_widget(Paragraph::new(flash_line(msg)), row);
    }
    log_scroll
}

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
