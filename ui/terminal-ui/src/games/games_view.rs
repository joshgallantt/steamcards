//! The games pop-up: picking which games are farmed first.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::Ctx,
    popup::{fit_height, modal, toggle},
    theme::{self, BUSY},
    widgets::{fit, hints, scroll, selected_row, spread},
};

pub(crate) struct GamesView {
    pub(crate) cursor: usize,
}

pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &GamesView) {
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
        let right =
            crate::onboarding::onboarding_view::game_facts(r, inner_w.saturating_sub(1 + 4 + 16));
        let row = spread(left, right, inner_w);
        lines.push(if i == cursor {
            selected_row(row, inner_w)
        } else {
            row
        });
    }
    let status = crate::onboarding::onboarding_view::library_status(cx, inner_w);
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
