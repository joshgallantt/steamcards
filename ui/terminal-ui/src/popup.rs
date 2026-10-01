//! What every pop-up shares: a framed box in the middle of the screen,
//! sized to what it holds, and the lines most of them show.

use ratatui::{
    Frame,
    layout::Rect,
    style::Color,
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Padding},
};

use crate::{
    app::Ctx,
    theme::{self, ACCENT, BUSY, GOOD},
    widgets::{centered, keycap},
};

pub(crate) fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

/// A line with a space either side, to sit in a border.
pub(crate) fn pad(mut l: Line<'static>) -> Line<'static> {
    l.spans.insert(0, Span::raw(" "));
    l.spans.push(Span::raw(" "));
    l
}

/// A centred pop-up frame `w`×`h` (borders included) with key hints in its
/// bottom edge. Returns the area inside it.
pub(crate) fn modal(
    f: &mut Frame<'_>,
    area: Rect,
    w: u16,
    h: u16,
    title: &str,
    keys: Line<'static>,
) -> Rect {
    modal_in(f, area, w, h, title, keys, ACCENT)
}

/// `modal` with a chosen border colour (the details pop-up uses the selection
/// colour, like the panel it stands in for).
pub(crate) fn modal_in(
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
pub(crate) fn fit_height(lines: usize) -> u16 {
    lines as u16 + 4
}

pub(crate) fn spinner_line(cx: &Ctx<'_>, text: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{} ", cx.spinner()), theme::fg(BUSY)),
        dim(text.to_owned()),
    ])
}

/// "◉ label [k]": an option that's on or off, with the key that flips it.
pub(crate) fn toggle(on: bool, label: &str, key: &str) -> Line<'static> {
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
