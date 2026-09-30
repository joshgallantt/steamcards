// Quit? (docs/design/ui.md §2.2): while farming, quitting asks first, as
// today: the cards that dropped stay in the Steam inventory.

use ratatui::{layout::Rect, text::Line};

use super::{
    super::{
        text::{Fits, Hint, wrap},
        theme,
    },
    Shown,
};

const KEYS: [Hint; 2] = [Hint::new("y", "quit", 0), Hint::new("n", "keep farming", 0)];

pub(super) fn shown(area: Rect) -> Fits<Shown> {
    let w = usize::from(area.width).saturating_sub(6);
    let mut lines = wrap(Line::styled("Stop farming and quit?", theme::bold()), w, 0)?;
    lines.push(Line::default());
    lines.extend(wrap(
        Line::styled(
            "Cards that dropped stay in your Steam inventory.",
            theme::dim(),
        ),
        w,
        0,
    )?);
    Ok(Shown::new("Quit?", lines, &KEYS))
}
