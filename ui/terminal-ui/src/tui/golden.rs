// The contract between the spec and the screens. Each mockup in §3 of
// docs/design/ui.md is an exact render of the spec's data set; a golden test
// renders the same screen at the mockup's size, from the same data set,
// into ratatui's test backend, and compares them character for character,
// colour aside. A keycap is written as the mockups write it: "[k]", its
// cap's first and last cells as the brackets.
//
// The spec is read at compile time, so a changed mockup rebuilds the tests.
// A mismatch prints a line-by-line diff.

use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color, text::Line};

use super::{App, text::NB, theme};

const SPEC: &str = include_str!("../../../../docs/design/ui.md");

/// A mockup from the spec: its size, its title and its rows, trailing
/// spaces trimmed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mockup {
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) title: String,
    pub(crate) rows: Vec<String>,
}

/// Every mockup in the spec, in order.
pub(crate) fn mockups() -> Vec<Mockup> {
    let mut out = Vec::new();
    let mut lines = SPEC.lines();
    while let Some(line) = lines.next() {
        let Some(info) = line.strip_prefix("```mockup ") else {
            continue;
        };
        let (size, title) = info.split_once(' ').unwrap_or((info, ""));
        let (w, h) = size
            .split_once(['x', '×'])
            .unwrap_or_else(|| panic!("a mockup's size: {size:?}"));
        let rows = lines
            .by_ref()
            .take_while(|l| l.trim_end() != "```")
            .map(|l| l.trim_end().to_owned())
            .collect();
        out.push(Mockup {
            width: w.trim().parse().expect("a width"),
            height: h.trim().parse().expect("a height"),
            title: title.trim().to_owned(),
            rows,
        });
    }
    out
}

/// The mockup with this title.
pub(crate) fn mockup(title: &str) -> Mockup {
    let all = mockups();
    all.iter()
        .find(|m| m.title == title)
        .cloned()
        .unwrap_or_else(|| {
            let titles: Vec<&str> = all.iter().map(|m| m.title.as_str()).collect();
            panic!("no mockup titled {title:?}; the spec has {titles:#?}")
        })
}

/// Whether a cell is a keycap's: a dark key on a light grey cap.
fn is_keycap(fg: Color, bg: Color) -> bool {
    let cap = theme::keycap();
    Some(fg) == cap.fg && Some(bg) == cap.bg
}

/// A screen as the mockups write it: a keycap's run of cells as "[k]", no-
/// break spaces as spaces, each row's trailing spaces trimmed.
pub(crate) fn screen_text(buf: &Buffer) -> Vec<String> {
    let area = buf.area;
    (area.top()..area.bottom())
        .map(|y| {
            let cells: Vec<(String, bool)> = (area.left()..area.right())
                .map(|x| {
                    let cell = &buf[(x, y)];
                    (cell.symbol().replace(NB, " "), is_keycap(cell.fg, cell.bg))
                })
                .collect();
            let mut row = String::new();
            for (i, (symbol, cap)) in cells.iter().enumerate() {
                let starts = *cap && (i == 0 || !cells[i - 1].1);
                let ends = *cap && cells.get(i + 1).is_none_or(|c| !c.1);
                row.push_str(match (starts, ends) {
                    (true, _) => "[",
                    (_, true) => "]",
                    _ => symbol,
                });
            }
            row.trim_end().to_owned()
        })
        .collect()
}

/// A line as the mockups write it, keycaps as "[k]".
pub(crate) fn line_text(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|span| {
            let text = span.content.replace(NB, " ");
            let style = line.style.patch(span.style);
            match (style.fg, style.bg) {
                (Some(fg), Some(bg)) if is_keycap(fg, bg) && text.chars().count() >= 2 => {
                    let inner: String = text
                        .chars()
                        .skip(1)
                        .take(text.chars().count() - 2)
                        .collect();
                    format!("[{inner}]")
                }
                _ => text,
            }
        })
        .collect()
}

/// Renders `app` at `w` × `h`.
pub(crate) fn render(app: &mut App, w: u16, h: u16) -> Buffer {
    let mut terminal = Terminal::new(TestBackend::new(w, h)).expect("a test terminal");
    app.sync_selection();
    terminal.draw(|f| app.draw(f)).expect("a frame");
    terminal.backend().buffer().clone()
}

/// The rows that differ, as a diff to read: each with the spec's row above
/// the screen's.
fn diff(want: &[String], got: &[String]) -> String {
    let mut out = String::new();
    for i in 0..want.len().max(got.len()) {
        let (w, g) = (want.get(i), got.get(i));
        if w != g {
            out.push_str(&format!(
                "row {i:>2} spec   │{}\n       screen │{}\n",
                w.map_or("(none)", String::as_str),
                g.map_or("(none)", String::as_str)
            ));
        }
    }
    out
}

/// Renders `app` at the size of the mockup titled `title`, and fails with a
/// diff unless the screen reads exactly as the mockup does.
pub(crate) fn check(title: &str, app: &mut App) {
    let m = mockup(title);
    let got = screen_text(&render(app, m.width, m.height));
    assert!(
        got == m.rows,
        "the screen isn't the mockup {title:?} ({}×{}):\n{}",
        m.width,
        m.height,
        diff(&m.rows, &got)
    );
}

#[cfg(test)]
mod tests {
    use ratatui::{layout::Rect, style::Style, text::Span};

    use super::*;
    use crate::tui::{fixtures, text};

    #[test]
    fn every_mockup_is_read_at_its_size() {
        let all = mockups();
        assert!(all.len() >= 36, "{} mockups", all.len());
        for m in &all {
            assert_eq!(m.rows.len(), usize::from(m.height), "{}", m.title);
            assert!(!m.title.is_empty());
        }
        let small = mockup("too small");
        assert_eq!((small.width, small.height), (50, 12));
        assert_eq!(small.rows[4], "         it needs 60×16, and it's 50×12.");
    }

    #[test]
    #[should_panic(expected = "no mockup titled")]
    fn a_title_the_spec_doesnt_have_fails() {
        mockup("no such screen");
    }

    #[test]
    fn a_keycap_reads_as_the_mockups_write_it() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 1));
        let line = Line::from(vec![
            Span::raw(" "),
            text::key("?"),
            Span::raw(" help "),
            text::key(" 0 "),
        ]);
        text::put(&mut buf, 0, 0, 20, &line);
        assert_eq!(screen_text(&buf), [" [?] help [ 0 ]"]);
        assert_eq!(line_text(&line), " [?] help [ 0 ]");
        assert_eq!(line_text(&Line::styled("plain", Style::new())), "plain");
    }

    #[test]
    fn a_mismatch_says_which_rows_differ() {
        let want = vec!["same".to_owned(), "spec".to_owned()];
        let got = vec!["same".to_owned(), "screen".to_owned(), "extra".to_owned()];
        assert_eq!(
            diff(&want, &got),
            "row  1 spec   │spec\n       screen │screen\nrow  2 spec   │(none)\n       screen │extra\n"
        );
    }

    #[tokio::test]
    async fn the_too_small_screen_is_the_spec_s() {
        let mut app = fixtures::farming_alone();
        check("too small", &mut app);
    }
}
