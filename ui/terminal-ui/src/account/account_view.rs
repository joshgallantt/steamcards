//! The account pop-up: who's signed in, signing in again, and signing out.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::Ctx,
    popup::{fit_height, modal},
    theme::{self, BAD, BUSY, GOOD},
    widgets::{fit, hints, selected_row, spread, wrap_text},
};

pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, confirm: bool) {
    // As wide as it can be, up to W: what's in it wraps to fit.
    const W: u16 = 72;
    let inner_w = W.min(area.width).saturating_sub(6) as usize;
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
    left.extend(crate::dashboard::dashboard_view::account_badge(cx));
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
    lines.extend(
        wrap_text(
            "Appearing offline, and keeping steamcards up to date, are in settings.",
            inner_w,
        )
        .into_iter()
        .map(|l| Line::styled(l, theme::dim())),
    );

    let enter = if cx.signed_in() {
        "sign in again"
    } else {
        "sign in"
    };
    let mut keys = vec![("enter", enter, 0), ("s", "settings", 2)];
    if cx.signed_in() {
        keys.push(("d", "sign out", 1));
    }
    keys.push(("esc", "close", 0));
    let keys = hints(&keys, inner_w);
    let inner = modal(f, area, W, fit_height(lines.len()), "Account", keys);
    f.render_widget(Paragraph::new(lines), inner);
}
