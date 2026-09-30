// The account (docs/design/ui.md, mockup m): who's signed in and what farming
// is doing, signing in again with the Steam app, how friends see the games,
// the wallet's currency every price is shown in, and that the computer is
// kept awake while games play. Signing out asks first (d, then y).

use ratatui::{
    layout::Rect,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx,
        text::{Fits, Hint, first_fit, fit, join, key, spread, wrap},
        theme,
    },
    Shown, chosen,
};
use crate::viewmodel::{AccountView, Run};

/// The facts' labels: "Wallet  ", "Awake   ".
const LABEL: usize = 8;

pub(super) fn shown(cx: &Ctx<'_>, area: Rect, confirm: bool) -> Fits<Shown> {
    let w = usize::from(area.width).saturating_sub(6);
    let view = AccountView::build(&cx.s);
    if confirm {
        return sign_out(&view, w);
    }
    let dim = theme::dim();
    let mut badge = vec![
        Span::raw("› "),
        Span::styled("Steam", theme::steam()),
        Span::raw("  "),
    ];
    let state = match (&view.name, view.expired) {
        (None, _) => {
            badge.push(Span::styled("not signed in", dim));
            Line::default()
        }
        (Some(name), true) => {
            badge.push(Span::styled(
                format!("✕ {}", name_or(name)),
                theme::fg(theme::BAD),
            ));
            Line::styled("sign in again", theme::fg(theme::BAD))
        }
        (Some(name), false) => {
            badge.push(Span::styled("●", theme::fg(theme::GOOD)));
            badge.push(Span::raw(format!(" {}", name_or(name))));
            match view.run {
                Run::Running => Line::styled("farming", theme::fg(theme::GOOD)),
                Run::Paused => Line::styled("paused", theme::fg(theme::BUSY)),
                Run::Stopped => Line::styled("not farming", dim),
            }
        }
    };
    let mut lines = vec![
        chosen(spread(Line::from(badge), state, w, 1)?),
        Line::default(),
    ];
    lines.extend(wrap(
        Line::styled(
            "Signs in with the Steam app on your phone: scan a QR code and approve.",
            dim,
        ),
        w,
        0,
    )?);
    lines.push(Line::default());
    let offline = !view.appear_online;
    let mark = if offline {
        Span::styled(" ◉ ", theme::strong(theme::GOOD))
    } else {
        Span::styled(" ○ ", dim)
    };
    lines.push(first_fit(
        [
            Line::from(vec![
                mark.clone(),
                Span::raw("Appear offline while farming  "),
                key("v"),
            ]),
            Line::from(vec![mark, Span::raw("Appear offline  "), key("v")]),
        ],
        w,
    )?);
    for l in wrap(
        Line::styled(
            if offline {
                "Friends don't see the games being played. Steam counts them just the same."
            } else {
                "Off: friends see you online, and every game being played."
            },
            dim,
        ),
        w.saturating_sub(3),
        0,
    )? {
        lines.push(join([Line::from("   "), l]));
    }
    lines.push(Line::default());
    let wallet = match view.currency {
        Some(c) => {
            let symbol = c.symbol().map_or_else(String::new, |s| format!("{s} "));
            let code = c.code().map_or_else(|| c.to_string(), str::to_owned);
            format!("{symbol}{code}. Every price and value is shown in it, and never converted.")
        }
        None => "Not known yet: Steam says which currency once signed in, and prices wait for it."
            .to_owned(),
    };
    let awake =
        "The computer is kept awake while games play, and may sleep when nothing is played.";
    for (label, text) in [("Wallet", wallet.as_str()), ("Awake", awake)] {
        for (i, l) in wrap(text.to_owned(), w.saturating_sub(LABEL), 0)?
            .into_iter()
            .enumerate()
        {
            let head = if i == 0 {
                fit(Line::styled(label, theme::heading()), LABEL)?
            } else {
                Line::from(" ".repeat(LABEL))
            };
            lines.push(join([head, l]));
        }
    }
    let enter = if view.name.is_some() {
        "sign in again"
    } else {
        "sign in"
    };
    let mut keys = vec![Hint::new("enter", enter, 0)];
    if view.name.is_some() {
        keys.push(Hint::new("d", "sign out", 0));
    }
    keys.push(Hint::new(
        "v",
        if offline { "online" } else { "offline" },
        1,
    ));
    keys.push(Hint::new("esc", "close", 0));
    Ok(Shown::new("Account", lines, &keys))
}

/// The account's name, or what to call it when Steam didn't say.
fn name_or(name: &str) -> String {
    if name.is_empty() {
        "signed in".to_owned()
    } else {
        name.to_owned()
    }
}

/// "Sign out of Steam (alice)?", and what that does.
fn sign_out(view: &AccountView, w: usize) -> Fits<Shown> {
    let who = view
        .name
        .as_deref()
        .filter(|n| !n.is_empty())
        .map_or_else(String::new, |n| format!(" ({n})"));
    let mut lines = wrap(
        Line::from(vec![
            Span::styled("Sign out of Steam", theme::strong(theme::BAD)),
            Span::raw(format!("{who}?")),
        ]),
        w,
        0,
    )?;
    lines.push(Line::default());
    lines.extend(wrap(
        Line::styled(
            "Farming stops, and this session ends: its cards stay in your Steam inventory. \
             Signing in again takes a new QR scan.",
            theme::dim(),
        ),
        w,
        0,
    )?);
    let keys = [Hint::new("y", "sign out", 0), Hint::new("n", "keep", 0)];
    Ok(Shown::new("Account", lines, &keys))
}
