// Text as the screens write it (docs/design/ui.md §5.1): how wide it is,
// padded to a width, wrapped at spaces with figures and keycaps kept whole,
// game names shortened at a word boundary, and the rules, key hints, pips,
// gauges and big digits the screens are drawn with. It ports the toolkit the
// spec's mockups were drawn with, so the screens follow the same rules.
//
// Nothing is cut. Text that doesn't fit its room is an `Overflow`, which a
// ladder answers by trying something shorter. Every write to the screen goes
// through `put`, which fails a test on text wider than its area rather than
// let ratatui clip it, and pads what's shorter.
//
// A space that must not break, inside a figure ("≈ 4d 21h") or a keycap, is a
// no-break space, `NB`: it measures and wraps as part of its word, and `put`
// writes it as an ordinary space.

use std::fmt;

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
};

use super::theme;

/// A space that doesn't break a line: inside a figure, and around a keycap's
/// letter.
pub(crate) const NB: char = '\u{a0}';

/// Text wider than the room it was given: a ladder's cue to try something
/// shorter, and a bug if it reaches the screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Overflow(pub(crate) String);

impl fmt::Display for Overflow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Text that fits, or why it doesn't.
pub(crate) type Fits<T> = Result<T, Overflow>;

// ── Measuring ────────────────────────────────────────────────────────────────

/// How many columns `s` takes on screen.
pub(crate) fn width(s: &str) -> usize {
    Span::raw(s).width()
}

/// A line's text, as it reads: a keycap as its letter with a space either
/// side, a no-break space as a space.
pub(crate) fn plain(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<String>()
        .replace(NB, " ")
}

/// A figure made one word, so a wrap never splits it: "≈ 4d 21h",
/// "≥ £1.45", "80%: 3d 13h – 6d 15h".
pub(crate) fn glue(s: &str) -> String {
    s.replace(' ', &NB.to_string())
}

fn too_wide(line: &Line<'_>, w: usize) -> Overflow {
    Overflow(format!(
        "{:?} is {} wide, over {w}",
        plain(line),
        line.width()
    ))
}

fn gap(n: usize) -> Span<'static> {
    Span::raw(" ".repeat(n))
}

// ── Fitting ──────────────────────────────────────────────────────────────────

/// `line` padded on the right to exactly `w` columns; never cut.
pub(crate) fn fit(line: impl Into<Line<'static>>, w: usize) -> Fits<Line<'static>> {
    let mut line = line.into();
    let n = line.width();
    if n > w {
        return Err(too_wide(&line, w));
    }
    if n < w {
        line.spans.push(gap(w - n));
    }
    Ok(line)
}

/// `line` padded on the left to exactly `w` columns: flush right.
pub(crate) fn rfit(line: impl Into<Line<'static>>, w: usize) -> Fits<Line<'static>> {
    let mut line = line.into();
    let n = line.width();
    if n > w {
        return Err(too_wide(&line, w));
    }
    if n < w {
        line.spans.insert(0, gap(w - n));
    }
    Ok(line)
}

/// `line` centred in exactly `w` columns, any odd column on the right.
pub(crate) fn cfit(line: impl Into<Line<'static>>, w: usize) -> Fits<Line<'static>> {
    let mut line = line.into();
    let n = line.width();
    if n > w {
        return Err(too_wide(&line, w));
    }
    let left = (w - n) / 2;
    if left > 0 {
        line.spans.insert(0, gap(left));
    }
    if w - n - left > 0 {
        line.spans.push(gap(w - n - left));
    }
    Ok(line)
}

/// `left` flush left and `right` flush right in exactly `w` columns, at least
/// `min_gap` apart.
pub(crate) fn spread(
    left: impl Into<Line<'static>>,
    right: impl Into<Line<'static>>,
    w: usize,
    min_gap: usize,
) -> Fits<Line<'static>> {
    let (mut left, right) = (left.into(), right.into());
    let (l, r) = (left.width(), right.width());
    if l + min_gap + r > w {
        return Err(Overflow(format!(
            "spread: {:?} + {:?} over {w}",
            plain(&left),
            plain(&right)
        )));
    }
    left.spans.push(gap(w - l - r));
    left.spans.extend(right.spans);
    Ok(left)
}

/// Joins pieces into one line, in order.
pub(crate) fn join(pieces: impl IntoIterator<Item = impl Into<Line<'static>>>) -> Line<'static> {
    let mut out = Line::default();
    for piece in pieces {
        out.spans.extend(piece.into().spans);
    }
    out
}

/// The first of `options`, longest first, that fits in `w` columns.
pub(crate) fn first_fit(
    options: impl IntoIterator<Item = impl Into<Line<'static>>>,
    w: usize,
) -> Fits<Line<'static>> {
    let mut last = None;
    for option in options {
        let option = option.into();
        if option.width() <= w {
            return Ok(option);
        }
        last = Some(plain(&option));
    }
    Err(Overflow(format!("nothing fits in {w}: {last:?}")))
}

// ── Wrapping and shortening ──────────────────────────────────────────────────

/// One word of a styled line, and the style of the space before it.
struct Word {
    spans: Vec<Span<'static>>,
    space: Style,
}

impl Word {
    fn width(&self) -> usize {
        self.spans.iter().map(Span::width).sum()
    }
}

/// A styled line cut into its words at ordinary spaces. A no-break space, in
/// a figure or a keycap, is part of its word.
fn words(line: &Line<'static>) -> Vec<Word> {
    let mut out = Vec::new();
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut space = Style::new();
    let mut pending = Style::new();
    for span in &line.spans {
        let style = line.style.patch(span.style);
        let mut text = String::new();
        for ch in span.content.chars() {
            if ch == ' ' {
                if !text.is_empty() {
                    spans.push(Span::styled(std::mem::take(&mut text), style));
                }
                if !spans.is_empty() {
                    out.push(Word {
                        spans: std::mem::take(&mut spans),
                        space,
                    });
                }
                pending = style;
            } else {
                if text.is_empty() && spans.is_empty() {
                    space = pending;
                }
                text.push(ch);
            }
        }
        if !text.is_empty() {
            spans.push(Span::styled(text, style));
        }
    }
    if !spans.is_empty() {
        out.push(Word { spans, space });
    }
    out
}

/// A word longer than a line: only ever a URL, which breaks after a "/"
/// (§5.1), into pieces of at most `room` columns. Any other word that long
/// is an overflow.
fn break_url(word: Word, room: usize) -> Fits<Vec<Word>> {
    let text: String = word.spans.iter().map(|s| s.content.as_ref()).collect();
    let style = word.spans.first().map_or(Style::new(), |s| s.style);
    if !text.contains('/') {
        return Err(Overflow(format!("word {text:?} longer than {room}")));
    }
    let mut pieces = Vec::new();
    let mut cur = String::new();
    for ch in text.chars() {
        cur.push(ch);
        if width(&cur) > room {
            let Some(at) = cur.rfind('/').map(|i| i + 1).filter(|&at| at < cur.len()) else {
                return Err(Overflow(format!("{text:?} has no break within {room}")));
            };
            let rest = cur.split_off(at);
            pieces.push(std::mem::replace(&mut cur, rest));
        }
    }
    pieces.push(cur);
    Ok(pieces
        .into_iter()
        .enumerate()
        .map(|(i, p)| Word {
            spans: vec![Span::styled(p, style)],
            space: if i == 0 { word.space } else { Style::new() },
        })
        .collect())
}

/// `text` wrapped to `w` columns at ordinary spaces, whole words only; the
/// lines after the first start `indent` columns in. A figure glued with
/// no-break spaces, and a keycap, never break. A word longer than a line
/// breaks after a "/" if it's a URL, and is an overflow otherwise: it is
/// never cut.
pub(crate) fn wrap(
    text: impl Into<Line<'static>>,
    w: usize,
    indent: usize,
) -> Fits<Vec<Line<'static>>> {
    let text = text.into();
    let room = w.saturating_sub(indent);
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut cur_w = 0;
    let mut started = false;
    for word in words(&text) {
        // The very first line has no indent, so a word as wide as the line
        // fits there.
        let fresh = if lines.is_empty() && !started {
            w
        } else {
            room
        };
        let pieces = if word.width() > fresh.max(room) || (started && word.width() > room) {
            break_url(word, room)?
        } else {
            vec![word]
        };
        for piece in pieces {
            let n = piece.width();
            if started && cur_w + 1 + n <= w {
                cur.push(Span::styled(" ", piece.space));
                cur.extend(piece.spans);
                cur_w += 1 + n;
                continue;
            }
            if started {
                lines.push(Line::from(std::mem::take(&mut cur)));
            }
            let lead = if lines.is_empty() { 0 } else { indent };
            if lead + n > w {
                return Err(Overflow(format!("a word is {n} wide, over {}", w - lead)));
            }
            if lead > 0 {
                cur.push(gap(lead));
            }
            cur.extend(piece.spans);
            cur_w = lead + n;
            started = true;
        }
    }
    lines.push(Line::from(cur));
    Ok(lines)
}

/// A game's name in a table cell of `n` columns: whole, or shortened at a
/// word boundary with "…" and no trailing punctuation, "Warhammer 40,000:
/// Dawn of…". The details always show the full name. A name whose first word
/// doesn't fit can't be shortened.
pub(crate) fn shorten(name: &str, n: usize) -> Fits<String> {
    if width(name) <= n {
        return Ok(name.to_owned());
    }
    let trim = |s: &str| s.trim_end_matches([' ', '-', ':', ',']).to_owned();
    let mut out = String::new();
    for word in name.split(' ') {
        let cand = if out.is_empty() {
            word.to_owned()
        } else {
            format!("{out} {word}")
        };
        if width(&trim(&cand)) + 1 > n {
            break;
        }
        out = cand;
    }
    let out = trim(&out);
    if out.is_empty() {
        return Err(Overflow(format!(
            "can't shorten {name:?} to {n} at a word boundary"
        )));
    }
    Ok(format!("{out}…"))
}

// ── Keycaps, rules and key hints ─────────────────────────────────────────────

/// A keycap: a dark key on a light grey cap, " k ", as wide as the "[k]"
/// the mockups write. Its spaces don't break.
pub(crate) fn key(k: &str) -> Span<'static> {
    Span::styled(format!("{NB}{}{NB}", glue(k)), theme::keycap())
}

/// "── TITLE ────────── hint ─", exactly `w` wide: the rule and its lead-in
/// dim, the title and hint as given. The hint goes if it won't fit with at
/// least three columns of rule.
pub(crate) fn rule(
    title: impl Into<Line<'static>>,
    w: usize,
    hint: Option<Line<'static>>,
) -> Fits<Line<'static>> {
    let title = title.into();
    let head = 3 + title.width() + 1;
    if let Some(hint) = hint {
        let tail = 1 + hint.width() + 2;
        if head + 3 + tail <= w {
            let mut out = rule_head(title);
            out.spans
                .push(Span::styled("─".repeat(w - head - tail), theme::dim()));
            out.spans.push(Span::raw(" "));
            out.spans.extend(hint.spans);
            out.spans.push(Span::styled(" ─", theme::dim()));
            return Ok(out);
        }
    }
    if head + 1 > w {
        return Err(Overflow(format!("rule {:?} over {w}", plain(&title))));
    }
    let mut out = rule_head(title);
    out.spans
        .push(Span::styled("─".repeat(w - head), theme::dim()));
    Ok(out)
}

fn rule_head(title: Line<'static>) -> Line<'static> {
    let mut out = Line::from(Span::styled("── ", theme::dim()));
    out.spans.extend(title.spans);
    out.spans.push(Span::raw(" "));
    out
}

/// A rule whose hint is the longest of `hints` that fits, or none.
pub(crate) fn rule_ladder(
    title: impl Into<Line<'static>>,
    w: usize,
    hints: impl IntoIterator<Item = impl Into<Line<'static>>>,
) -> Fits<Line<'static>> {
    let title = title.into();
    let head = 3 + title.width() + 1;
    for hint in hints {
        let hint = hint.into();
        if head + 3 + 1 + hint.width() + 2 <= w {
            return rule(title, w, Some(hint));
        }
    }
    rule(title, w, None)
}

/// One key hint: the key, what it does, and how much it matters (lower
/// matters more). A key of `DIVIDER` separates groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Hint {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) priority: u8,
}

/// Divides groups of hints.
pub(crate) const DIVIDER: &str = "│";

impl Hint {
    pub(crate) const fn new(key: &'static str, label: &'static str, priority: u8) -> Self {
        Self {
            key,
            label,
            priority,
        }
    }

    pub(crate) const fn divider(priority: u8) -> Self {
        Self::new(DIVIDER, "", priority)
    }

    fn is_divider(&self) -> bool {
        self.key == DIVIDER
    }

    fn cost(&self) -> usize {
        if self.is_divider() {
            1
        } else {
            width(self.key)
                + 2
                + if self.label.is_empty() {
                    0
                } else {
                    1 + width(self.label)
                }
        }
    }
}

/// Key hints in `w` columns: keycaps with dim labels, "[q] quit". Gaps
/// tighten from 3 to 2, then the least important go, the rest keeping their
/// order: the first that doesn't fit ends the row. A divider shows only
/// between two groups that both stay.
pub(crate) fn hints(pairs: &[Hint], w: usize) -> Fits<Line<'static>> {
    let full = |gap: usize| {
        pairs.iter().map(Hint::cost).sum::<usize>() + gap * pairs.len().saturating_sub(1)
    };
    let gap = if full(3) <= w { 3 } else { 2 };
    let mut order: Vec<usize> = (0..pairs.len()).collect();
    order.sort_by_key(|&i| pairs[i].priority);
    let mut keep = vec![false; pairs.len()];
    let mut used = 0;
    for i in order {
        let extra = pairs[i].cost() + if used > 0 { gap } else { 0 };
        if used + extra > w {
            break;
        }
        keep[i] = true;
        used += extra;
    }
    let kept: Vec<usize> = (0..pairs.len()).filter(|&i| keep[i]).collect();
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut last_divider = true;
    for (n, &i) in kept.iter().enumerate() {
        let hint = pairs[i];
        let at_end = kept[n + 1..].iter().all(|&j| pairs[j].is_divider());
        if hint.is_divider() && (last_divider || at_end) {
            continue;
        }
        if !out.is_empty() {
            out.push(gap_span(gap));
        }
        if hint.is_divider() {
            out.push(Span::styled(DIVIDER, theme::dim()));
        } else {
            out.push(key(hint.key));
            if !hint.label.is_empty() {
                out.push(Span::styled(format!(" {}", hint.label), theme::dim()));
            }
        }
        last_divider = hint.is_divider();
    }
    let line = Line::from(out);
    if line.width() > w {
        return Err(too_wide(&line, w));
    }
    Ok(line)
}

fn gap_span(n: usize) -> Span<'static> {
    gap(n)
}

/// Whether every hint fits in `w` columns, none dropped.
#[cfg_attr(not(test), expect(dead_code, reason = "placed by the pop-up stage"))]
pub(crate) fn all_fit(pairs: &[Hint], w: usize) -> bool {
    pairs.iter().map(Hint::cost).sum::<usize>() + 2 * pairs.len().saturating_sub(1) <= w
}

/// Hints as lines, as many to a line as fit, for keys that don't all fit in
/// a pop-up's border.
#[cfg_attr(not(test), expect(dead_code, reason = "placed by the pop-up stage"))]
pub(crate) fn pack_hints(pairs: &[Hint], w: usize) -> Fits<Vec<Line<'static>>> {
    let mut out = Vec::new();
    let mut cur: Vec<Hint> = Vec::new();
    for &hint in pairs {
        let mut with = cur.clone();
        with.push(hint);
        if !cur.is_empty() && !all_fit(&with, w) {
            out.push(hints(&cur, w)?);
            cur = vec![hint];
        } else {
            cur = with;
        }
    }
    if !cur.is_empty() {
        out.push(hints(&cur, w)?);
    }
    Ok(out)
}

// ── Pips, gauges and big digits ──────────────────────────────────────────────

/// One pip per drop: ● had before this session, ◆ dropped this session, ★ a
/// foil this session, ○ still to come (§5.5).
pub(crate) fn pips(before: u32, today: u32, foils: u32, total: u32) -> Line<'static> {
    let normal = today.saturating_sub(foils);
    let to_come = total.saturating_sub(before + today);
    let mut spans = Vec::new();
    let mut push = |n: u32, glyph: &str, style: Style| {
        if n > 0 {
            spans.push(Span::styled(glyph.repeat(n as usize), style));
        }
    };
    push(before, "●", Style::new());
    push(normal, "◆", theme::fg(theme::GOOD));
    push(foils, "★", theme::fg(theme::ACCENT));
    push(to_come, "○", theme::dim());
    Line::from(spans)
}

/// A thin gauge `w` wide: ━ done, ─ to go. Anything done shows at least one
/// ━, and anything left at least one ─.
pub(crate) fn gauge(done: u32, of: u32, w: usize, done_style: Style) -> Line<'static> {
    let frac = if of == 0 {
        0.0
    } else {
        f64::from(done) / f64::from(of)
    };
    let mut n = (frac * w as f64).round_ties_even() as usize;
    if frac > 0.0 && n == 0 {
        n = 1;
    }
    if frac < 1.0 && n == w {
        n = w.saturating_sub(1);
    }
    let n = n.min(w);
    Line::from(vec![
        Span::styled("━".repeat(n), done_style),
        Span::styled("─".repeat(w - n), theme::dim()),
    ])
}

/// Three rows of box-drawing strokes for the big time to finish at L (§5.5):
/// "4d 21h".
pub(crate) fn big(text: &str) -> [String; 3] {
    let glyph = |ch: char| -> [&str; 3] {
        match ch {
            '0' => ["┌─┐", "│ │", "└─┘"],
            '1' => ["╶┐ ", " │ ", "╶┴╴"],
            '2' => ["╶─┐", "┌─┘", "└─╴"],
            '3' => ["╶─┐", " ─┤", "╶─┘"],
            '4' => ["╷ ╷", "└─┤", "  ╵"],
            '5' => ["┌─╴", "└─┐", "╶─┘"],
            '6' => ["┌─╴", "├─┐", "└─┘"],
            '7' => ["╶─┐", "  │", "  ╵"],
            '8' => ["┌─┐", "├─┤", "└─┘"],
            '9' => ["┌─┐", "└─┤", "╶─┘"],
            'd' => ["  ╷", "┌─┤", "└─┘"],
            'h' => ["╷  ", "├─┐", "╵ ╵"],
            'm' => ["   ", "┌┬┐", "╵╵╵"],
            _ => ["  ", "  ", "  "],
        }
    };
    let chars: Vec<char> = text.chars().collect();
    let mut rows = [String::new(), String::new(), String::new()];
    for (i, &ch) in chars.iter().enumerate() {
        let g = glyph(ch);
        for (row, part) in rows.iter_mut().zip(g) {
            row.push_str(part);
        }
        let next = chars.get(i + 1);
        if ch != ' ' && next.is_some_and(|&n| n != ' ') {
            for row in &mut rows {
                row.push(' ');
            }
        }
    }
    rows
}

// ── Writing to the screen ────────────────────────────────────────────────────

/// Writes `line` at (`x`, `y`), padded with spaces in the line's own style to
/// exactly `w` columns. The one way text reaches the screen: a line wider
/// than `w` means a ladder let something through. A test fails on it; the
/// running app, which must keep farming, draws only what fits.
pub(crate) fn put(buf: &mut Buffer, x: u16, y: u16, w: u16, line: &Line<'_>) {
    let n = line.width();
    if n > usize::from(w) && cfg!(test) {
        panic!(
            "text at ({x}, {y}) is {n} wide, over {w}: {:?}",
            plain(line)
        );
    }
    let area = *buf.area();
    if y < area.top() || y >= area.bottom() || x >= area.right() {
        if cfg!(test) {
            panic!(
                "text at ({x}, {y}) is outside the screen: {:?}",
                plain(line)
            );
        }
        return;
    }
    let w = w.min(area.right() - x);
    let mut col = x;
    let end = x + w;
    for span in &line.spans {
        if col >= end {
            break;
        }
        let text = span.content.replace(NB, " ");
        let (next, _) = buf.set_stringn(
            col,
            y,
            &text,
            usize::from(end - col),
            line.style.patch(span.style),
        );
        col = next;
    }
    if col < end {
        buf.set_stringn(
            col,
            y,
            " ".repeat(usize::from(end - col)),
            usize::MAX,
            line.style,
        );
    }
}

/// Writes `lines` down `area`, one a row from its top, each padded to its
/// width. More lines than rows is a ladder's mistake too.
pub(crate) fn put_lines(buf: &mut Buffer, area: Rect, lines: &[Line<'_>]) {
    if lines.len() > usize::from(area.height) && cfg!(test) {
        panic!(
            "{} lines in {} rows, from {:?}",
            lines.len(),
            area.height,
            lines.first().map(plain)
        );
    }
    for (row, line) in (area.y..area.bottom()).zip(lines) {
        put(buf, area.x, row, area.width, line);
    }
}

/// What a rounded panel says in its borders: a title top left and a note
/// top right, and bottom left and right. Each is given as it's drawn, without
/// the spaces either side, which the panel adds.
#[derive(Debug, Clone, Default)]
pub(crate) struct Titles {
    pub(crate) top_left: Option<Line<'static>>,
    pub(crate) top_right: Option<Line<'static>>,
    pub(crate) bottom_left: Option<Line<'static>>,
    pub(crate) bottom_right: Option<Line<'static>>,
}

/// Whether a border `w` wide has room for `left` and `right` titles: each
/// with a space either side, a corner and a column of rule at each end, and
/// a column of rule between them.
pub(crate) fn titles_fit(w: usize, left: Option<&Line<'_>>, right: Option<&Line<'_>>) -> bool {
    let used = |t: Option<&Line<'_>>| t.map_or(0, |t| t.width() + 2);
    match (left, right) {
        (_, None) => used(left) + 4 <= w,
        (None, Some(_)) => used(right) + 4 <= w,
        (Some(_), Some(_)) => 1 + used(left) + 1 + used(right) + 2 <= w,
    }
}

/// A rounded panel over `area`: its border in `border`, the titles in its
/// border as `╭ Title ──── note ─╮`. The inside isn't touched. A title that
/// doesn't fit is a ladder's mistake, as any text is.
pub(crate) fn panel(buf: &mut Buffer, area: Rect, border: Style, titles: &Titles) {
    let (w, h) = (usize::from(area.width), area.height);
    if w < 2 || h < 2 {
        if cfg!(test) {
            panic!("a panel {w}×{h} is too small");
        }
        return;
    }
    let edge =
        |l: char, r: char| Line::from(Span::styled(format!("{l}{}{r}", "─".repeat(w - 2)), border));
    put(buf, area.x, area.y, area.width, &edge('╭', '╮'));
    put(buf, area.x, area.bottom() - 1, area.width, &edge('╰', '╯'));
    for y in area.y + 1..area.bottom() - 1 {
        buf.set_string(area.x, y, "│", border);
        buf.set_string(area.right() - 1, y, "│", border);
    }
    border_titles(
        buf,
        area,
        area.y,
        titles.top_left.as_ref(),
        titles.top_right.as_ref(),
    );
    border_titles(
        buf,
        area,
        area.bottom() - 1,
        titles.bottom_left.as_ref(),
        titles.bottom_right.as_ref(),
    );
}

fn border_titles(
    buf: &mut Buffer,
    area: Rect,
    y: u16,
    left: Option<&Line<'static>>,
    right: Option<&Line<'static>>,
) {
    let w = usize::from(area.width);
    if !titles_fit(w, left, right) {
        if cfg!(test) {
            panic!(
                "border titles {:?} and {:?} over {w}",
                left.map(plain),
                right.map(plain)
            );
        }
        return;
    }
    let padded = |t: &Line<'static>| {
        let mut l = Line::from(Span::raw(" "));
        l.spans.extend(t.spans.clone());
        l.spans.push(Span::raw(" "));
        l
    };
    if let Some(t) = left {
        let t = padded(t);
        put(buf, area.x + 1, y, t.width() as u16, &t);
    }
    if let Some(t) = right {
        let t = padded(t);
        let tw = t.width() as u16;
        put(buf, area.right() - 2 - tw, y, tw, &t);
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::{Color, Modifier};

    use super::*;

    fn text(line: &Line<'_>) -> String {
        plain(line)
    }

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines.iter().map(plain).collect()
    }

    #[test]
    fn text_is_padded_to_its_width_and_never_cut() {
        assert_eq!(text(&fit("abc", 5).unwrap()), "abc  ");
        assert_eq!(text(&rfit("abc", 5).unwrap()), "  abc");
        assert_eq!(text(&cfit("abc", 6).unwrap()), " abc  ");
        assert!(fit("abcdef", 5).is_err());
        assert!(rfit("abcdef", 5).is_err());
        assert_eq!(
            text(&spread("left", "right", 12, 1).unwrap()),
            "left   right"
        );
        assert!(spread("left", "right", 9, 1).is_err());
        assert_eq!(width("≈ 4d 21h · ●◆★○ … ×2 — ⠋"), 24);
    }

    #[test]
    fn the_first_that_fits_is_taken() {
        let options = ["a long option", "short", "s"];
        assert_eq!(text(&first_fit(options, 10).unwrap()), "short");
        assert_eq!(text(&first_fit(options, 1).unwrap()), "s");
        assert!(first_fit(options, 0).is_err());
    }

    #[test]
    fn prose_wraps_at_spaces_with_figures_whole() {
        let line = format!(
            "The time to finish is {} with a band {}",
            glue("≈ 4d 21h"),
            glue("80%: 3d 13h – 6d 15h")
        );
        let lines = wrap(line, 30, 0).unwrap();
        assert_eq!(
            texts(&lines),
            [
                "The time to finish is ≈ 4d 21h",
                "with a band",
                "80%: 3d 13h – 6d 15h"
            ]
        );
        assert!(lines.iter().all(|l| l.width() <= 30));
    }

    #[test]
    fn a_wrap_indents_what_follows() {
        let lines = wrap(
            "2 this session: Madison at 17:05, and one at 17:23 still being identified",
            50,
            7,
        )
        .unwrap();
        assert_eq!(
            texts(&lines),
            [
                "2 this session: Madison at 17:05, and one at 17:23",
                "       still being identified"
            ]
        );
    }

    #[test]
    fn a_keycap_is_one_word() {
        let line = Line::from(vec![
            Span::raw("Press "),
            key("enter"),
            Span::raw(" to agree and begin."),
        ]);
        let lines = wrap(line, 12, 0).unwrap();
        assert_eq!(
            texts(&lines),
            ["Press", " enter  to", "agree and", "begin."]
        );
        let cap = &lines[1].spans[0];
        assert_eq!(cap.style, theme::keycap(), "the keycap keeps its style");
    }

    #[test]
    fn a_word_longer_than_a_line_is_never_cut() {
        assert!(wrap("a supercalifragilistic word", 10, 0).is_err());
        let url = wrap(
            "Visit https://steamcommunity.com/my/gamecards/960910/ to see it",
            24,
            0,
        )
        .unwrap();
        assert_eq!(
            texts(&url),
            [
                "Visit https://",
                "steamcommunity.com/my/",
                "gamecards/960910/ to see",
                "it"
            ],
            "a URL breaks after a slash"
        );
    }

    #[test]
    fn a_wrap_keeps_each_words_style() {
        let line = Line::from(vec![
            Span::styled("≥ £1.45", theme::bold()),
            Span::raw(" · "),
            Span::styled("3 unpriced", theme::fg(theme::BUSY)),
        ]);
        let lines = wrap(line, 40, 0).unwrap();
        assert_eq!(text(&lines[0]), "≥ £1.45 · 3 unpriced");
        let styled: Vec<(String, Style)> = lines[0]
            .spans
            .iter()
            .map(|s| (s.content.to_string(), s.style))
            .collect();
        assert!(styled.contains(&("£1.45".to_owned(), theme::bold())));
        assert!(styled.contains(&("unpriced".to_owned(), theme::fg(theme::BUSY))));
    }

    #[test]
    fn a_game_name_shortens_at_a_word_boundary() {
        let warhammer = "Warhammer 40,000: Dawn of War II - Anniversary Edition";
        assert_eq!(
            shorten(warhammer, 40).unwrap(),
            "Warhammer 40,000: Dawn of War II…"
        );
        assert_eq!(
            shorten(warhammer, 49).unwrap(),
            "Warhammer 40,000: Dawn of War II - Anniversary…"
        );
        assert_eq!(
            shorten(warhammer, 26).unwrap(),
            "Warhammer 40,000: Dawn of…"
        );
        assert_eq!(
            shorten("We Were Here Expeditions: The FriendShip", 26).unwrap(),
            "We Were Here Expeditions…",
            "no trailing colon"
        );
        assert_eq!(
            shorten("We Were Here Expeditions: The FriendShip", 32).unwrap(),
            "We Were Here Expeditions: The…"
        );
        assert_eq!(shorten("Heavy Rain", 10).unwrap(), "Heavy Rain");
        assert!(shorten("Supercalifragilistic", 8).is_err());
    }

    #[test]
    fn a_keycap_is_as_wide_as_the_mockups_write_it() {
        assert_eq!(key("enter").width(), "[enter]".len());
        assert_eq!(key(" 0 ").width(), "[ 0 ]".len());
        assert_eq!(key("↑↓").width(), 4);
        assert_eq!(key("?").style.bg, Some(Color::Gray));
    }

    #[test]
    fn a_rule_fills_its_width_and_drops_a_hint_that_doesnt_fit() {
        let r = rule("DROPS", 40, Some(Line::from("a long hint"))).unwrap();
        assert_eq!(
            text(&r),
            format!("── DROPS {} a long hint ─", "─".repeat(17))
        );
        assert_eq!(r.width(), 40);
        let r = rule_ladder("DROPS", 24, ["a long hint", "hint"]).unwrap();
        assert_eq!(text(&r), format!("── DROPS {} hint ─", "─".repeat(8)));
        let r = rule_ladder("DROPS", 14, ["a long hint", "hint"]).unwrap();
        assert_eq!(text(&r), format!("── DROPS {}", "─".repeat(5)));
        assert!(rule("DROPS", 9, None).is_err());
    }

    #[test]
    fn the_spec_s_section_rules() {
        let w = 120;
        let r = rule_ladder(
            "INDIFFERENT · 57 · farming",
            w,
            [
                "≈ £15.34 left · after your priorities · closest to dropping first",
                "≈ £15.34 left · closest to dropping first",
                "≈ £15.34 left",
            ],
        )
        .unwrap();
        assert_eq!(
            text(&r),
            "── INDIFFERENT · 57 · farming ────────────────────── ≈ £15.34 left · after your \
             priorities · closest to dropping first ─"
        );
    }

    #[test]
    fn hints_drop_the_least_important_first_and_keep_their_order() {
        let pairs = [
            Hint::new("a", "one", 2),
            Hint::new("b", "two", 0),
            Hint::new("c", "three", 1),
        ];
        let text = |w| plain(&hints(&pairs, w).unwrap());
        assert_eq!(text(40), " a  one    b  two    c  three");
        assert_eq!(text(27), " a  one   b  two   c  three");
        assert_eq!(text(20), " b  two   c  three");
    }

    #[test]
    fn hints_stop_at_the_first_that_doesnt_fit() {
        let pairs = [
            Hint::new("a", "a very long label", 1),
            Hint::new("b", "b", 2),
        ];
        assert_eq!(
            plain(&hints(&pairs, 10).unwrap()),
            "",
            "b would fit alone, but a comes first"
        );
    }

    #[test]
    fn a_divider_shows_only_between_two_groups() {
        let pairs = [
            Hint::new("a", "one", 1),
            Hint::divider(0),
            Hint::new("b", "two", 0),
        ];
        let text = |w| plain(&hints(&pairs, w).unwrap());
        assert_eq!(text(40), " a  one   │    b  two");
        assert_eq!(text(10), " b  two");
    }

    #[test]
    fn keys_that_dont_all_fit_a_border_go_on_lines_of_their_own() {
        let keys = [
            Hint::new("enter", "sign in again", 0),
            Hint::new("d", "sign out", 0),
            Hint::new("v", "online", 1),
            Hint::new("esc", "close", 0),
        ];
        assert!(!all_fit(&keys, 38));
        assert!(all_fit(&keys, 60));
        let lines = pack_hints(&keys, 38).unwrap();
        assert_eq!(
            texts(&lines),
            [
                " enter  sign in again    d  sign out",
                " v  online    esc  close"
            ]
        );
    }

    #[test]
    fn pieces_join_into_one_line() {
        let line = join([Line::from("≈ "), Line::from(key("b")), Line::from(" basis")]);
        assert_eq!(text(&line), "≈  b  basis");
        assert_eq!(line.spans.len(), 3);
    }

    #[test]
    fn pips_count_each_drop() {
        assert_eq!(text(&pips(1, 2, 0, 4)), "●◆◆○");
        assert_eq!(text(&pips(0, 4, 1, 4)), "◆◆◆★");
        assert_eq!(text(&pips(3, 0, 0, 5)), "●●●○○");
        let styles: Vec<Style> = pips(1, 2, 1, 5).spans.iter().map(|s| s.style).collect();
        assert_eq!(
            styles,
            [
                Style::new(),
                theme::fg(theme::GOOD),
                theme::fg(theme::ACCENT),
                theme::dim()
            ]
        );
    }

    #[test]
    fn a_gauge_shows_a_little_of_anything() {
        assert_eq!(
            text(&gauge(16, 252, 41, Style::new())),
            format!("━━━{}", "─".repeat(38))
        );
        assert_eq!(
            text(&gauge(183, 421, 41, Style::new()))
                .matches('━')
                .count(),
            18
        );
        assert_eq!(text(&gauge(1, 1000, 10, Style::new())), "━─────────");
        assert_eq!(text(&gauge(999, 1000, 10, Style::new())), "━━━━━━━━━─");
        assert_eq!(text(&gauge(0, 252, 4, Style::new())), "────");
    }

    #[test]
    fn big_digits_are_three_rows_of_strokes() {
        assert_eq!(
            big("4d 21h"),
            [
                "╷ ╷   ╷  ╶─┐ ╶┐  ╷  ".to_owned(),
                "└─┤ ┌─┤  ┌─┘  │  ├─┐".to_owned(),
                "  ╵ └─┘  └─╴ ╶┴╴ ╵ ╵".to_owned(),
            ]
        );
    }

    fn row(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width)
            .map(|x| buf[(x, y)].symbol().to_owned())
            .collect()
    }

    #[test]
    fn put_pads_to_the_width_and_writes_no_break_spaces_as_spaces() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
        buf.set_string(0, 0, "xxxxxxxxxxxx", Style::new());
        let line = Line::from(vec![Span::raw("a"), key("b")]).style(theme::selected());
        put(&mut buf, 1, 0, 8, &line);
        assert_eq!(row(&buf, 0), "xa b     xxx");
        assert!(
            buf[(8, 0)].modifier.contains(Modifier::REVERSED),
            "padded in the line's style"
        );
        assert_eq!(buf[(3, 0)].bg, Color::Gray, "the keycap");
    }

    #[test]
    fn lines_go_down_an_area_a_row_each() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
        put_lines(
            &mut buf,
            Rect::new(1, 1, 4, 2),
            &[Line::from("ab"), Line::from("cd")],
        );
        assert_eq!(row(&buf, 0), "      ");
        assert_eq!(row(&buf, 1), " ab   ");
        assert_eq!(row(&buf, 2), " cd   ");
    }

    #[test]
    #[should_panic(expected = "3 lines in 2 rows")]
    fn more_lines_than_rows_is_refused() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 6, 3));
        put_lines(
            &mut buf,
            Rect::new(0, 0, 6, 2),
            &[Line::from("a"), Line::from("b"), Line::from("c")],
        );
    }

    #[test]
    #[should_panic(expected = "over 3")]
    fn put_refuses_text_wider_than_its_area() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 12, 1));
        put(&mut buf, 0, 0, 3, &Line::from("abcd"));
    }

    #[test]
    fn a_panel_carries_its_titles_in_its_borders() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 30, 3));
        let titles = Titles {
            top_left: Some(Line::from("Now")),
            top_right: Some(Line::from("since 16:53")),
            bottom_left: Some(Line::from("left")),
            bottom_right: None,
        };
        let area = buf.area;
        panel(&mut buf, area, theme::border(), &titles);
        assert_eq!(
            row(&buf, 0),
            format!("╭ Now {} since 16:53 ─╮", "─".repeat(9))
        );
        assert_eq!(row(&buf, 1), format!("│{}│", " ".repeat(28)));
        assert_eq!(row(&buf, 2), format!("╰ left {}╯", "─".repeat(22)));
        assert!(titles_fit(
            22,
            titles.top_left.as_ref(),
            titles.top_right.as_ref()
        ));
        assert!(!titles_fit(
            21,
            titles.top_left.as_ref(),
            titles.top_right.as_ref()
        ));
    }
}
