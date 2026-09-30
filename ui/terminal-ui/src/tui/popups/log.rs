// The log (docs/design/ui.md, "The log"): every event, the newest at the
// bottom, following the newest as they come until scrolled back. Each line
// is its time on the local clock (to the second where there's room), its
// glyph and what happened; one too long for a line wraps, under its words.
// The border counts events, never rows.

use crossterm::event::KeyCode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Span},
};

use super::{
    super::{
        Ctx, Logged, format,
        layout::strip::glyph_style,
        text::{Fits, Hint, NB, first_fit, hints, key, width, wrap},
        theme,
    },
    frame,
};

/// Where the log is scrolled to: following the newest (`None`), or from an
/// event on; and what the last draw showed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(in super::super) struct LogView {
    top: Option<usize>,
    /// The first and last events shown, and how many that was.
    first: usize,
    last: usize,
    shown: usize,
}

/// From this wide inside, times show their seconds.
const SECONDS: usize = 70;

const KEYS: [Hint; 3] = [
    Hint::new("↑↓", "scroll", 1),
    Hint::new("End", "follow", 2),
    Hint::new("esc", "close", 0),
];

impl LogView {
    /// Scrolls for a key: ↑↓ an event, PgUp PgDn a page, Home the oldest,
    /// End back to following the newest.
    pub(super) fn key(&mut self, code: KeyCode, events: usize) {
        let page = self.shown.max(1);
        let newest = events.saturating_sub(1);
        self.top = match code {
            KeyCode::Up => Some(self.first.saturating_sub(1)),
            KeyCode::PageUp => Some(self.first.saturating_sub(page)),
            KeyCode::Down if self.top.is_some() && self.last < newest => Some(self.first + 1),
            KeyCode::PageDown if self.top.is_some() && self.last + page < newest => {
                Some(self.first + page)
            }
            KeyCode::Down | KeyCode::PageDown | KeyCode::End => None,
            KeyCode::Home => Some(0),
            _ => self.top,
        };
    }
}

/// An event's line, `w` wide: "17:23:42  ✓ A card dropped for Heavy Rain —
/// 1 to go", wrapping under its words.
fn event(e: &Logged, cx: &Ctx<'_>, seconds: bool, w: usize) -> Fits<Vec<Line<'static>>> {
    let at = e.entry.at;
    let time = if seconds {
        let clock = format::clock(at, cx.s.now, cx.s.zone);
        let secs = at.with_timezone(&cx.s.zone).format(":%S").to_string();
        format!("{clock}{secs}")
    } else {
        format::clock(at, cx.s.now, cx.s.zone)
    };
    // The time, its glyph and the first word are one: the line never breaks
    // between them.
    let lead = width(&time) + 4;
    let line = Line::from(vec![
        Span::styled(format!("{time}{NB}{NB}"), theme::dim()),
        Span::styled(e.entry.kind.glyph(), glyph_style(e.entry.kind)),
        Span::raw(format!(" {}", e.entry.text)),
    ]);
    wrap(line, w, lead)
}

/// Draws the log over `area`: the newest events that fit, or those from
/// where it's scrolled to.
pub(super) fn draw(buf: &mut Buffer, cx: &Ctx<'_>, area: Rect, v: &mut LogView) -> Fits<()> {
    let w = usize::from(area.width);
    let inner = w.saturating_sub(6);
    let room = usize::from(area.height).saturating_sub(2);
    let log = &cx.app.log;
    let seconds = inner >= SECONDS;
    let mut rows: Vec<Line<'static>> = Vec::new();
    let mut shown = 0;
    let (mut first, mut last) = (log.len(), log.len().saturating_sub(1));
    if let Some(top) = v.top.filter(|&t| t < log.len()) {
        first = top;
        for (i, e) in log.iter().enumerate().skip(top) {
            let lines = event(e, cx, seconds, inner)?;
            if rows.len() + lines.len() > room && shown > 0 {
                break;
            }
            rows.extend(lines);
            last = i;
            shown += 1;
        }
        // Scrolled on to the newest, it follows them again.
        if last + 1 >= log.len() {
            v.top = None;
        }
    } else {
        v.top = None;
        for (i, e) in log.iter().enumerate().rev() {
            let lines = event(e, cx, seconds, inner)?;
            if rows.len() + lines.len() > room && shown > 0 {
                break;
            }
            let mut before = lines;
            before.extend(rows);
            rows = before;
            first = i;
            shown += 1;
        }
    }
    let first = first.min(log.len().saturating_sub(1));
    rows.truncate(room);
    v.first = first;
    v.last = last;
    v.shown = shown;
    if log.is_empty() {
        rows = vec![Line::styled("Nothing yet.", theme::dim())];
    }
    let earlier = first;
    let later = log.len().saturating_sub(last + 1);
    let following = v.top.is_none();
    let note = if following {
        format!("{} events · following the newest", log.len())
    } else {
        format!("{} events", log.len())
    };
    let mut counts = Vec::new();
    if earlier > 0 {
        counts.push(format!("{earlier} earlier ↑"));
    }
    if later > 0 {
        counts.push(format!("{later} later ↓"));
    }
    let counts = counts.join(" · ");
    let lead = |rest: Line<'static>| {
        if counts.is_empty() {
            rest
        } else {
            let mut l = Line::from(format!("{counts}   "));
            l.spans.extend(rest.spans);
            l
        }
    };
    let bottom = first_fit(
        [
            lead(hints(&KEYS, w.saturating_sub(30))?),
            lead(Line::from(vec![
                key("esc"),
                Span::styled(" close", theme::dim()),
            ])),
            Line::from(counts.clone()),
        ],
        w.saturating_sub(8),
    )?;
    frame(
        buf,
        area,
        Line::styled("Log", theme::heading()),
        Some(Line::styled(note, theme::dim())),
        Some(bottom),
        &rows,
    );
    Ok(())
}
