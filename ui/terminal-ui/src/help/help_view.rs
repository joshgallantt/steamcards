//! The help pop-up: every key, and what the colours and marks mean.

use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::Ctx,
    popup::{fit_height, modal},
    theme::{self, BAD, BUSY, GOOD},
    widgets::{hints, keycap, wrap_text},
};

pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>) {
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

    let mut farmed = vec![
        head("What gets farmed"),
        num("1", "Your priority games (1–9), #1 first"),
        num("2", "Games whose cards can drop now, fewest left first"),
        num("3", "Games still building hours, most played first"),
    ];
    let mut never = vec!["Skipped games (x)"];
    if cx.prefs.skip_private {
        never.push("private games");
    }
    if cx.prefs.skip_refundable {
        never.push("games you could still refund");
    }
    let never = match never.split_last() {
        Some((last, [])) => format!("{last} aren't farmed."),
        Some((last, rest)) => format!("{} and {last} aren't farmed.", rest.join(", ")),
        None => String::new(),
    };
    let drop = match cx.prefs.hours_before_drops {
        0 => "Cards drop for one game at a time, from the start on this account: each \
              game is farmed on its own."
            .to_owned(),
        n => format!(
            "Cards drop for one game at a time, once it has {} on record. Games short of \
             that play together, up to 32, to build hours.",
            if n == 1 {
                "an hour".to_owned()
            } else {
                format!("{n} hours")
            }
        ),
    };
    let dim_lines = |text: &str, indent: &str, w: usize| {
        wrap_text(text, w)
            .into_iter()
            .map(|l| Line::styled(format!("{indent}{l}"), theme::dim()))
            .collect::<Vec<_>>()
    };
    farmed.extend(dim_lines(&never, "    ", 53));
    farmed.push(Line::default());
    farmed.extend(dim_lines(&drop, " ", 56));
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
            head("Games"),
            row("↑↓", "choose a game (PgUp/PgDn jump)"),
            row("enter", "its details: each card and its price"),
            row("1-9", "rank the selected game"),
            row("0", "back to indifferent"),
            row("x", "skip it"),
            row("o", "open its card page"),
            row("c", "show or hide finished games"),
        ];
        let everywhere = vec![
            head("Everywhere"),
            row("a", "your account: sign in or out"),
            row("g", "games: pick priority games"),
            row("s", "settings, and how friends see you"),
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
        Line::default(),
        head("Prices"),
        Line::styled(
            " What buyers pay on the Steam market, in your wallet's",
            theme::dim(),
        ),
        Line::styled(
            " currency. £1.45+ means some cards aren't priced yet;",
            theme::dim(),
        ),
        Line::styled(
            " … is a price on its way, — is none to be had.",
            theme::dim(),
        ),
    ]
}
