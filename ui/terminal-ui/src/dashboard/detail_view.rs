//! The chosen game's details as a pop-up, in windows too narrow to show
//! them beside the games.

use ratatui::{Frame, layout::Rect, text::Line, widgets::Paragraph};

use crate::{
    app::Ctx,
    popup::{fit_height, modal_in},
    theme::SELECT,
    widgets::{hints, truncate},
};

/// The chosen game's details, scrolled `scroll` lines down when they don't
/// fit; returns the scroll it was drawn at, and the most it can be.
pub(crate) fn render(
    f: &mut Frame<'_>,
    area: Rect,
    cx: &Ctx<'_>,
    scroll: usize,
) -> Option<(usize, usize)> {
    let e = cx.selected()?;
    let w = area.width.saturating_sub(4).min(78);
    let inner_w = w.saturating_sub(6) as usize;
    let mut lines = crate::dashboard::dashboard_view::detail_info(e, cx, inner_w);
    lines.push(Line::default());
    lines.extend(crate::dashboard::dashboard_view::tier_controls(e, inner_w));
    let h = fit_height(lines.len()).min(area.height);
    // Inside: the borders, the padding row on top, and a blank row below.
    let room = (h as usize).saturating_sub(4);
    let max = lines.len().saturating_sub(room);
    let off = scroll.min(max);
    let below = lines.len().saturating_sub(off + room);
    let mut keys = vec![("↑↓", "previous / next game", 1), ("esc", "close", 0)];
    let more = format!("{below} more ↓");
    if below > 0 {
        keys.insert(0, ("PgDn", more.as_str(), 0));
    } else if off > 0 {
        keys.insert(0, ("PgUp", "back up", 2));
    }
    let keys = hints(&keys, inner_w);
    let name = truncate(&e.game.name, inner_w.saturating_sub(2));
    let inner = modal_in(f, area, w, h, &name, keys, SELECT);
    f.render_widget(Paragraph::new(lines).scroll((off as u16, 0)), inner);
    Some((off, max))
}
