// Small rendering building blocks shared by the dashboard and the pop-ups.

use std::time::Duration;

use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, BorderType, Padding},
};

use super::theme;

const PARTIAL: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];

/// Progress bar with eighth-of-a-cell resolution on a dim track.
pub(super) fn bar(current: i32, required: i32, width: usize, fill: Style) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let frac = if required > 0 {
        (current.max(0) as f64 / required as f64).min(1.0)
    } else {
        0.0
    };
    let eighths = (frac * (width * 8) as f64).round() as usize;
    let mut filled = "█".repeat(eighths / 8);
    let mut used = eighths / 8;
    if !eighths.is_multiple_of(8) && used < width {
        filled.push(PARTIAL[eighths % 8]);
        used += 1;
    }
    vec![
        Span::styled(filled, fill),
        Span::styled("░".repeat(width - used), theme::dim()),
    ]
}

/// "1h 12m", "12m", "<1m".
pub(super) fn elapsed(d: Duration) -> String {
    let m = d.as_secs() / 60;
    match m {
        0 => "<1m".to_owned(),
        m if m < 60 => format!("{m}m"),
        m => format!("{}h {}m", m / 60, m % 60),
    }
}

/// Cuts to `n` characters, ending in "…" when shortened.
pub(super) fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_owned();
    }
    if n == 0 {
        return String::new();
    }
    let mut out: String = s.chars().take(n - 1).collect();
    out.push('…');
    out
}

/// Truncates or pads to exactly `n` characters.
pub(super) fn fit(s: &str, n: usize) -> String {
    format!("{:<n$}", truncate(s, n))
}

/// Truncates or left-pads to exactly `n` characters.
pub(super) fn fit_right(s: &str, n: usize) -> String {
    format!("{:>n$}", truncate(s, n))
}

pub(super) fn width(spans: &[Span<'_>]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

/// A line with `left` flush left and `right` flush right in `w` columns. The
/// left side is cut short if both don't fit.
pub(super) fn spread(
    mut left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
    w: usize,
) -> Line<'static> {
    let rw = width(&right);
    let room = w.saturating_sub(rw + 1);
    let mut lw = 0;
    left.retain_mut(|s| {
        let n = s.content.chars().count();
        if lw >= room {
            return false;
        }
        if lw + n > room {
            s.content = truncate(&s.content, room - lw).into();
        }
        lw += s.content.chars().count();
        true
    });
    let gap = w.saturating_sub(lw + rw);
    left.push(Span::raw(" ".repeat(gap)));
    left.extend(right);
    Line::from(left)
}

/// Word-wraps `text` to `w` columns (long words are cut).
pub(super) fn wrap_text(text: &str, w: usize) -> Vec<String> {
    let w = w.max(1);
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        let word = truncate(word, w);
        let len = cur.chars().count();
        if len > 0 && len + 1 + word.chars().count() > w {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&word);
    }
    if !cur.is_empty() || lines.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Lays out "a, b, c" across at most `max_lines` lines of `w` columns; if the
/// list doesn't fit, the last line ends with "+N more".
pub(super) fn wrap_list(items: &[String], w: usize, max_lines: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    for (i, item) in items.iter().enumerate() {
        let last = i + 1 == items.len();
        let piece = if last {
            item.clone()
        } else {
            format!("{item}, ")
        };
        let cur = lines.last().unwrap().chars().count();
        if cur > 0 && cur + piece.trim_end().chars().count() > w {
            if lines.len() == max_lines {
                let line = lines.last_mut().unwrap();
                line.push_str(&format!("+{} more", items.len() - i));
                return lines;
            }
            lines.push(String::new());
        }
        lines.last_mut().unwrap().push_str(&piece);
    }
    lines
}

/// How far to scroll `len` lines shown `visible` at a time so line `focus`
/// sits mid-view (or as near as the ends allow).
pub(super) fn scroll(focus: usize, len: usize, visible: usize) -> usize {
    if len > visible {
        focus.saturating_sub(visible / 2).min(len - visible)
    } else {
        0
    }
}

/// A centred `w`×`h` rect inside `area`, shrunk to fit.
pub(super) fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width);
    let h = h.min(area.height);
    Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    }
}

/// Rounded panel with a bold title.
pub(super) fn panel(title: Line<'static>) -> Block<'static> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(theme::border())
        .title(title)
        .padding(Padding::horizontal(1))
}

/// A key drawn as a keycap, e.g. " q " on a grey key.
pub(super) fn keycap(k: &str) -> Span<'static> {
    Span::styled(format!(" {k} "), theme::keycap())
}

/// A short-lived message: what a key just did, or that it didn't stick.
pub(super) fn flash_line(msg: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(" ▸ ", theme::strong(theme::SELECT)),
        Span::styled(msg.to_owned(), theme::bold()),
    ])
}

/// Divider between groups of hints.
pub(super) const DIVIDER: &str = "│";

/// Key hints drawn as keycaps with labels: "[↑↓] choose  [a] accounts".
/// `pairs` are (key, label, priority — 0 = most important); a key of
/// `DIVIDER` separates groups. When the line would overflow, gaps tighten
/// first, then the least important hints go — the rest keep their order.
pub(super) fn hints(pairs: &[(&str, &str, u8)], w: usize) -> Line<'static> {
    let cost = |(k, l, _): &(&str, &str, u8)| {
        if *k == DIVIDER {
            1
        } else {
            k.chars().count()
                + 2
                + if l.is_empty() {
                    0
                } else {
                    1 + l.chars().count()
                }
        }
    };
    let full: usize = pairs.iter().map(cost).sum();
    let gap = if full + 3 * pairs.len().saturating_sub(1) <= w {
        3
    } else {
        2
    };
    let mut order: Vec<usize> = (0..pairs.len()).collect();
    order.sort_by_key(|&i| pairs[i].2);
    let mut keep = vec![false; pairs.len()];
    let mut used = 0;
    for i in order {
        let extra = cost(&pairs[i]) + if used == 0 { 0 } else { gap };
        if used + extra <= w {
            keep[i] = true;
            used += extra;
        }
    }
    // A divider only makes sense between two groups that both survived.
    let kept: Vec<usize> = (0..pairs.len()).filter(|&i| keep[i]).collect();
    let mut spans = Vec::new();
    let mut last_was_divider = true;
    for (n, &i) in kept.iter().enumerate() {
        let (k, l, _) = pairs[i];
        let divider = k == DIVIDER;
        let at_end = kept[n + 1..].iter().all(|&j| pairs[j].0 == DIVIDER);
        if divider && (last_was_divider || at_end) {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        if divider {
            spans.push(Span::styled(DIVIDER, theme::dim()));
        } else {
            spans.push(keycap(k));
            if !l.is_empty() {
                spans.push(Span::styled(format!(" {l}"), theme::dim()));
            }
        }
        last_was_divider = divider;
    }
    Line::from(spans)
}

/// The first of `variants` (most to least detailed) that fits in `w`
/// columns, if any.
pub(super) fn first_fit(variants: Vec<Vec<Span<'static>>>, w: usize) -> Option<Vec<Span<'static>>> {
    variants.into_iter().find(|v| width(v) <= w)
}

/// A divider line: "── TITLE ─────────────── hint ─" with the rule dimmed and
/// the lead-in drawn in `lead`. `hints` go from most to least detailed; the
/// first that leaves room for some rule is used, or none at all — the line
/// never overflows `w`.
pub(super) fn rule(
    title: Vec<Span<'static>>,
    hints: Vec<Vec<Span<'static>>>,
    w: usize,
    lead: Style,
) -> Line<'static> {
    const MIN_FILL: usize = 3;
    let head = 3 + width(&title) + 1;
    let hint = first_fit(hints, w.saturating_sub(head + MIN_FILL + 3)).unwrap_or_default();
    let tail = if hint.is_empty() {
        0
    } else {
        1 + width(&hint) + 2
    };
    let mut spans = vec![Span::styled("── ", lead)];
    spans.extend(title);
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        "─".repeat(w.saturating_sub(head + tail)),
        theme::dim(),
    ));
    if !hint.is_empty() {
        spans.push(Span::raw(" "));
        spans.extend(hint);
        spans.push(Span::styled(" ─", theme::dim()));
    }
    Line::from(spans)
}

/// Paints a whole row as the selection bar, padded to `w` so the bar spans
/// the full width. Block-drawn progress bars become lines, since solid blocks
/// would read as holes in a reversed bar.
pub(super) fn selected_row(line: Line<'static>, w: usize) -> Line<'static> {
    let as_line = |c: char| match c {
        '█' | '▏' | '▎' | '▍' | '▌' | '▋' | '▊' | '▉' => '━',
        '░' => '─',
        c => c,
    };
    let mut spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|s| {
            Span::styled(
                s.content.chars().map(as_line).collect::<String>(),
                theme::selected(),
            )
        })
        .collect();
    let used = width(&spans);
    if used < w {
        spans.push(Span::styled(" ".repeat(w - used), theme::selected()));
    }
    Line::from(spans)
}

/// Half-block QR code rows, with a two-module quiet zone.
pub(super) fn qr(url: &str) -> Vec<String> {
    use qrcode::{Color as QrColor, EcLevel, QrCode};

    let Ok(code) = QrCode::with_error_correction_level(url, EcLevel::L) else {
        return Vec::new();
    };
    let n = code.width();
    let dark: Vec<bool> = code
        .to_colors()
        .into_iter()
        .map(|c| c == QrColor::Dark)
        .collect();
    const PAD: usize = 2;
    let size = n + PAD * 2;
    let at = |r: usize, c: usize| {
        (PAD..n + PAD).contains(&r)
            && (PAD..n + PAD).contains(&c)
            && dark[(r - PAD) * n + (c - PAD)]
    };
    (0..size)
        .step_by(2)
        .map(|r| {
            (0..size)
                .map(|c| match (at(r, c), at(r + 1, c)) {
                    (true, true) => '█',
                    (true, false) => '▀',
                    (false, true) => '▄',
                    (false, false) => ' ',
                })
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_and_fit() {
        assert_eq!(truncate("Rust Isles Boonie", 8), "Rust Is…");
        assert_eq!(truncate("abc", 8), "abc");
        assert_eq!(fit("abc", 5), "abc  ");
        assert_eq!(fit_right("abc", 5), "  abc");
    }

    #[test]
    fn bar_fills_proportionally() {
        let text = |spans: Vec<Span<'_>>| {
            spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert_eq!(text(bar(0, 60, 4, Style::new())), "░░░░");
        assert_eq!(text(bar(60, 60, 4, Style::new())), "████");
        assert_eq!(text(bar(30, 60, 4, Style::new())), "██░░");
        assert_eq!(text(bar(15, 60, 2, Style::new())), "▌░");
    }

    #[test]
    fn hints_drop_low_priority_first_but_keep_order() {
        let pairs = [("a", "one", 2), ("b", "two", 0), ("c", "three", 1)];
        let text = |w| {
            hints(&pairs, w)
                .spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert_eq!(text(40), " a  one    b  two    c  three");
        assert_eq!(text(27), " a  one   b  two   c  three");
        assert_eq!(text(20), " b  two   c  three");
    }

    #[test]
    fn hints_drop_orphaned_dividers() {
        let pairs = [("a", "one", 1), (DIVIDER, "", 0), ("b", "two", 0)];
        let text = |w| {
            hints(&pairs, w)
                .spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert_eq!(text(40), " a  one   │    b  two");
        assert_eq!(text(10), " b  two");
    }

    #[test]
    fn rule_fills_the_width_and_drops_hints_that_dont_fit() {
        let text = |w| {
            let hints = vec![vec![Span::raw("a long hint")], vec![Span::raw("hint")]];
            let line = rule(vec![Span::raw("DROPS")], hints, w, Style::new());
            line.spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        };
        assert_eq!(
            text(40),
            format!("── DROPS {} a long hint ─", "─".repeat(17))
        );
        assert_eq!(text(24), format!("── DROPS {} hint ─", "─".repeat(8)));
        assert_eq!(text(14), format!("── DROPS {}", "─".repeat(5)));
        assert_eq!(text(24).chars().count(), 24);
    }

    #[test]
    fn wrap_text_breaks_on_words() {
        assert_eq!(
            wrap_text("token validation failed: 401 Unauthorized", 16),
            ["token validation", "failed: 401", "Unauthorized",]
        );
        assert_eq!(wrap_text("", 10), [""]);
    }

    #[test]
    fn wrap_list_caps_lines_with_a_count() {
        let items: Vec<String> = ["fuslie", "abe", "ledoo", "cyr", "hjune"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(wrap_list(&items, 40, 3), ["fuslie, abe, ledoo, cyr, hjune"]);
        assert_eq!(
            wrap_list(&items, 12, 2),
            ["fuslie, abe, ", "ledoo, cyr, +1 more"]
        );
    }

    #[test]
    fn spread_right_aligns() {
        let line = spread(vec![Span::raw("left")], vec![Span::raw("right")], 12);
        let text: String = line.spans.iter().map(|s| s.content.to_string()).collect();
        assert_eq!(text, "left   right");
    }
}
