// The too-small screen (docs/design/ui.md mockup p): what size the window
// needs, and the glance farming keeps meanwhile, the state, "appears
// offline", the drops, the time to finish and the value. Every line is
// centred, and one that doesn't fit is left out whole.

use ratatui::{
    Frame,
    text::{Line, Span},
};

use super::{
    layout::{self, SMALLEST},
    text::{cfit, first_fit, put, spread},
    theme,
};
use crate::viewmodel::{Header, Now, Progress, Snapshot};

pub(super) fn render(f: &mut Frame<'_>, s: &Snapshot<'_>, spinner: &str) {
    let area = f.area();
    let (w, h) = (area.width, area.height);
    let buf = f.buffer_mut();
    let header = Header::build(s);
    let pill = Line::from(vec![
        Span::styled(" steamcards ", theme::pill()),
        Span::raw(" "),
        layout::state_word(&header),
    ]);
    let offline = Line::styled(
        if header.appear_online {
            "online "
        } else {
            "appears offline "
        },
        theme::dim(),
    );
    let top = spread(pill.clone(), offline, usize::from(w), 1)
        .or_else(|_| {
            first_fit(
                [
                    pill,
                    Line::from(Span::styled(" steamcards ", theme::pill())),
                ],
                usize::from(w),
            )
        })
        .unwrap_or_default();
    if h == 0 {
        return;
    }
    put(buf, 0, 0, w, &top);

    let progress = Progress::build(s, None);
    let now = Now::build(s);
    let mut lines = vec![
        Line::styled("Make the window a little bigger:", theme::bold()),
        Line::styled(
            format!("it needs {}×{}, and it's {w}×{h}.", SMALLEST.0, SMALLEST.1),
            theme::dim(),
        ),
        Line::default(),
        Line::from(if progress.activity.runs() {
            "Farming carries on meanwhile:"
        } else {
            "Meanwhile:"
        }),
    ];
    lines.extend(layout::glance(&progress, &now, spinner, usize::from(w)));
    lines.retain(|l| l.width() <= usize::from(w));
    lines.truncate(usize::from(h - 1));
    let y0 = 1 + (usize::from(h - 1) - lines.len()) / 2;
    for (i, line) in lines.into_iter().enumerate() {
        if let Ok(centred) = cfit(line, usize::from(w)) {
            put(buf, 0, (y0 + i) as u16, w, &centred);
        }
    }
}
