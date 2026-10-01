//! The log pop-up: everything the farmer and the market have said, newest
//! last.

use ratatui::{
    Frame,
    layout::Rect,
    text::Line,
    widgets::{Block, BorderType, Clear, Padding, Paragraph},
};

use crate::{
    app::Ctx,
    popup::{dim, pad},
    theme::{self},
    widgets::hints,
};

pub(crate) struct LogView {
    pub(crate) offset: usize,
    /// Largest offset at the last draw (the view's bottom).
    pub(crate) max: usize,
    /// Stick to the newest entries as they arrive.
    pub(crate) follow: bool,
}

pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &LogView) -> (usize, usize) {
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
            .map(|e| crate::dashboard::dashboard_view::log_line(e, false, inner.width as usize))
            .collect();
        f.render_widget(Paragraph::new(lines), inner);
    }
    (off, max)
}
