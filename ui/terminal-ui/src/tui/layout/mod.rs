// The layout rules (docs/design/ui.md §2.3–§2.5), ported from the generator
// that drew the spec's mockups: which size class a window is, where each
// region goes, what each region gives up as the window shrinks, and where a
// pop-up goes. Pure: sizes and view models go in, rectangles and lines come
// out, so each rule is checked on its own.

pub(crate) mod header;
pub(crate) mod progress;
pub(crate) mod queue;
pub(crate) mod right;
pub(crate) mod strip;

use ratatui::layout::Rect;

pub(crate) use header::state_word;
pub(crate) use progress::glance;
pub(crate) use right::haul_rows_m;

use super::text::{Fits, Hint, Overflow, all_fit, first_fit, hints, key, pack_hints};

/// A window's size class: the largest it meets in both width and height.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SizeClass {
    TooSmall,
    Xs,
    S,
    M,
    L,
}

const L: (u16, u16) = (200, 40);
const M: (u16, u16) = (100, 26);
const S: (u16, u16) = (72, 20);
/// Below this, the too-small screen.
pub(crate) const SMALLEST: (u16, u16) = (60, 16);

/// The right-hand column at L, before it takes spare width.
const RIGHT_L: u16 = 76;
/// The queue's name column never grows past this; spare columns go to the
/// right-hand column at L once it has.
pub(crate) const NAME_MAX: usize = 56;

pub(crate) fn size_class(w: u16, h: u16) -> SizeClass {
    let meets = |(mw, mh): (u16, u16)| w >= mw && h >= mh;
    if !meets(SMALLEST) {
        SizeClass::TooSmall
    } else if meets(L) {
        SizeClass::L
    } else if meets(M) {
        SizeClass::M
    } else if meets(S) {
        SizeClass::S
    } else {
        SizeClass::Xs
    }
}

/// The right-hand column's width: 76 at L, from 217 columns the spare too;
/// 40 to 66 at M; none below.
pub(crate) fn right_width(w: u16, class: SizeClass) -> u16 {
    let w = i32::from(w);
    let right = match class {
        SizeClass::L => i32::from(RIGHT_L).max(w - (4 + 80 + NAME_MAX as i32)),
        SizeClass::M => (46 + (w - 120).div_euclid(2)).clamp(40, 66),
        _ => 0,
    };
    u16::try_from(right).unwrap_or(0)
}

/// Where each region of the dashboard goes at a size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Regions {
    pub(crate) class: SizeClass,
    pub(crate) header: Rect,
    /// Progress's panel: 7 rows at L, 6 at M (7 from 36 rows), 5 at S. At
    /// XS, three bare rows.
    pub(crate) progress: Rect,
    /// Rows inside Progress: 5 at L, 4 or 5 at M, 3 below.
    pub(crate) progress_rows: u16,
    pub(crate) queue: Rect,
    /// The Now panel, at L only.
    pub(crate) now: Option<Rect>,
    /// The right-hand column below Now at L, beside the queue at M.
    pub(crate) right: Option<Rect>,
    /// Everything between the header and the strip.
    pub(crate) body: Rect,
    pub(crate) strip: Rect,
    pub(crate) footer: Rect,
}

/// The Now panel's height at L.
const NOW_ROWS: u16 = 8;

/// The dashboard's regions at `w` × `h`; `None` when it's too small.
pub(crate) fn regions(w: u16, h: u16) -> Option<Regions> {
    let class = size_class(w, h);
    let row = |y: u16| Rect::new(0, y, w, 1);
    // The body runs from under the header to above the strip.
    let bottom = h - 3;
    let body = Rect::new(0, 1, w, bottom);
    let (progress, progress_rows, queue, now, right) = match class {
        SizeClass::TooSmall => return None,
        SizeClass::L => {
            let rw = right_width(w, class);
            let qw = w - rw;
            let progress = Rect::new(0, 1, qw, 7);
            let queue = Rect::new(0, 8, qw, bottom - 7);
            let now = Rect::new(qw, 1, rw, NOW_ROWS);
            let right = Rect::new(qw, 1 + NOW_ROWS, rw, bottom - NOW_ROWS);
            (progress, 5, queue, Some(now), Some(right))
        }
        SizeClass::M => {
            let rows = if h >= 36 { 5 } else { 4 };
            let progress = Rect::new(0, 1, w, rows + 2);
            let top = 1 + rows + 2;
            let rw = right_width(w, class);
            let queue = Rect::new(0, top, w - rw, bottom - top + 1);
            let right = Rect::new(w - rw, top, rw, bottom - top + 1);
            (progress, rows, queue, None, Some(right))
        }
        SizeClass::S => {
            let progress = Rect::new(0, 1, w, 5);
            (progress, 3, Rect::new(0, 6, w, bottom - 5), None, None)
        }
        SizeClass::Xs => {
            let progress = Rect::new(1, 1, w - 1, 3);
            (progress, 3, Rect::new(0, 4, w, bottom - 3), None, None)
        }
    };
    Some(Regions {
        class,
        header: row(0),
        progress,
        progress_rows,
        queue,
        now,
        right,
        body,
        strip: row(h - 2),
        footer: row(h - 1),
    })
}

impl Regions {
    /// Whether the queue has its column header: not below 18 rows, and not
    /// at XS, whose columns are named in the border.
    pub(crate) fn queue_header(&self) -> bool {
        self.class != SizeClass::Xs && self.body.height + 3 >= 18
    }

    /// Rows inside the queue for its rules and games.
    pub(crate) fn queue_rows(&self) -> u16 {
        self.queue.height - 2 - u16::from(self.queue_header())
    }
}

/// Where a pop-up can go: the right-hand column (below Now at L) when its
/// content fits there, or the whole body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    Right,
    Body,
}

/// The pop-ups, for where each goes at each size (§2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Popup {
    Details,
    Account,
    SignIn,
    Quit,
    /// This session's cards, the market, games & settings, the log, help.
    Wide,
}

/// Where a pop-up goes: the right-hand column where the spec's table puts
/// it there, and the size has one; otherwise the whole body. A pop-up
/// covers whole panels, never part of one.
pub(crate) fn popup_area(popup: Popup, regions: &Regions) -> Rect {
    let place = match (popup, regions.class) {
        (Popup::Details | Popup::SignIn, SizeClass::L) => Place::Right,
        (Popup::Account | Popup::Quit, SizeClass::L | SizeClass::M) => Place::Right,
        _ => Place::Body,
    };
    match (place, regions.right) {
        (Place::Right, Some(right)) => right,
        _ => regions.body,
    }
}

/// A pop-up's inside and its bottom border, laid out: the lines that show,
/// the keys in the border when they all fit there or on its last lines when
/// they don't, and how much more is below when the text is longer than its
/// area.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placed {
    pub(crate) lines: Vec<ratatui::text::Line<'static>>,
    /// What the bottom border says, on its right.
    pub(crate) keys: Option<ratatui::text::Line<'static>>,
    /// Lines below, out of sight: "15 more ↓ [PgDn]".
    pub(crate) below: usize,
    /// How far down it's scrolled, kept within its lines, and how many lines
    /// a page shows.
    pub(crate) offset: usize,
    pub(crate) page: usize,
}

/// Lays `lines` out in a pop-up over `area`, with its `keys`, scrolled
/// `offset` lines down. Inside, text keeps 2 columns from each side.
pub(crate) fn place(
    area: Rect,
    lines: &[ratatui::text::Line<'static>],
    keys: &[Hint],
    offset: usize,
) -> Fits<Placed> {
    let (w, h) = (usize::from(area.width), usize::from(area.height));
    let inner = w.saturating_sub(6);
    let room = h.saturating_sub(2);
    if let Some(wide) = lines.iter().find(|l| l.width() > inner) {
        return Err(Overflow(format!(
            "a pop-up's line is {} wide, over {inner}",
            wide.width()
        )));
    }
    let border = w.saturating_sub(8);
    let in_border = all_fit(keys, border);
    let key_lines = if in_border {
        Vec::new()
    } else {
        pack_hints(keys, inner)?
    };
    let avail = room.saturating_sub(if key_lines.is_empty() {
        0
    } else {
        key_lines.len() + 1
    });
    if avail == 0 {
        return Err(Overflow("a pop-up with no room".into()));
    }
    let offset = offset.min(lines.len().saturating_sub(avail));
    let mut shown: Vec<ratatui::text::Line<'static>> =
        lines.iter().skip(offset).take(avail).cloned().collect();
    let below = lines.len().saturating_sub(offset + avail);
    let keys_line = if below > 0 {
        let more = ratatui::text::Line::from(vec![
            ratatui::text::Span::raw(format!("{below} more ↓ ")),
            key("PgDn"),
        ]);
        let with_keys = |spare: usize| -> Option<ratatui::text::Line<'static>> {
            let keys = hints(keys, spare).ok()?;
            let mut l = more.clone();
            l.spans.push(ratatui::text::Span::raw("   "));
            l.spans.extend(keys.spans);
            Some(l)
        };
        let spare = border.saturating_sub(more.width() + 3);
        let mut options = Vec::new();
        if in_border && all_fit(keys, spare) {
            options.extend(with_keys(spare));
        }
        let mut close = more.clone();
        close.spans.push(ratatui::text::Span::raw("   "));
        close.spans.push(key("esc"));
        close
            .spans
            .push(ratatui::text::Span::styled(" close", super::theme::dim()));
        options.push(close);
        options.push(more);
        Some(first_fit(options, border)?)
    } else if in_border && !keys.is_empty() {
        Some(hints(keys, border)?)
    } else {
        None
    };
    if !key_lines.is_empty() {
        shown.resize(avail, ratatui::text::Line::default());
        shown.push(ratatui::text::Line::default());
        shown.extend(key_lines);
    }
    Ok(Placed {
        lines: shown,
        keys: keys_line,
        below,
        offset,
        page: avail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_takes_the_largest_class_it_meets_both_ways() {
        let class = |w, h| size_class(w, h);
        assert_eq!(class(200, 40), SizeClass::L);
        assert_eq!(class(209, 49), SizeClass::L);
        assert_eq!(class(240, 39), SizeClass::M, "40 rows for L");
        assert_eq!(class(199, 70), SizeClass::M, "200 columns for L");
        assert_eq!(class(100, 26), SizeClass::M);
        assert_eq!(class(99, 30), SizeClass::S);
        assert_eq!(class(120, 25), SizeClass::S);
        assert_eq!(class(72, 20), SizeClass::S);
        assert_eq!(class(71, 20), SizeClass::Xs);
        assert_eq!(class(80, 19), SizeClass::Xs);
        assert_eq!(class(60, 16), SizeClass::Xs);
        assert_eq!(class(59, 16), SizeClass::TooSmall);
        assert_eq!(class(60, 15), SizeClass::TooSmall);
        assert_eq!(class(50, 12), SizeClass::TooSmall);
        assert!(SizeClass::L > SizeClass::M && SizeClass::Xs > SizeClass::TooSmall);
    }

    #[test]
    fn the_right_hand_column_is_76_at_l_and_40_to_66_at_m() {
        assert_eq!(right_width(200, SizeClass::L), 76);
        assert_eq!(right_width(209, SizeClass::L), 76);
        assert_eq!(right_width(216, SizeClass::L), 76);
        assert_eq!(
            right_width(217, SizeClass::L),
            77,
            "from 217, the spare too"
        );
        assert_eq!(right_width(146, SizeClass::M), 59);
        assert_eq!(right_width(120, SizeClass::M), 46);
        assert_eq!(right_width(101, SizeClass::M), 40);
        assert_eq!(right_width(100, SizeClass::M), 40);
        assert_eq!(right_width(199, SizeClass::M), 66);
        assert_eq!(right_width(80, SizeClass::S), 0);
    }

    /// The games the queue shows at a size: its rows, less the two rules
    /// (Priority, empty, and Indifferent) above its games.
    fn games_in_view(w: u16, h: u16) -> u16 {
        regions(w, h).unwrap().queue_rows() - 2
    }

    #[test]
    fn the_queue_shows_as_many_games_as_the_spec_says() {
        // §2.3's table.
        assert_eq!(games_in_view(209, 49), 34, "the user's window, as today");
        assert_eq!(games_in_view(200, 50), 35);
        assert_eq!(games_in_view(146, 40), 25);
        assert_eq!(games_in_view(120, 30), 16, "as today");
        assert_eq!(games_in_view(100, 26), 12);
        assert_eq!(games_in_view(80, 24), 11);
        assert_eq!(games_in_view(60, 16), 6);
    }

    #[test]
    fn each_region_has_its_place() {
        let l = regions(200, 50).unwrap();
        assert_eq!(l.progress, Rect::new(0, 1, 124, 7));
        assert_eq!(l.now, Some(Rect::new(124, 1, 76, 8)));
        assert_eq!(l.right, Some(Rect::new(124, 9, 76, 39)));
        assert_eq!(l.queue, Rect::new(0, 8, 124, 40));
        assert_eq!(
            (l.strip.y, l.footer.y, l.body),
            (48, 49, Rect::new(0, 1, 200, 47))
        );
        let m = regions(146, 40).unwrap();
        assert_eq!((m.progress, m.progress_rows), (Rect::new(0, 1, 146, 7), 5));
        assert_eq!(m.right, Some(Rect::new(87, 8, 59, 30)));
        let m = regions(120, 30).unwrap();
        assert_eq!((m.progress.height, m.progress_rows), (6, 4));
        assert_eq!(m.queue, Rect::new(0, 7, 74, 21));
        let s = regions(80, 24).unwrap();
        assert_eq!(
            (s.progress, s.right, s.now),
            (Rect::new(0, 1, 80, 5), None, None)
        );
        assert!(s.queue_header());
        let xs = regions(60, 16).unwrap();
        assert_eq!(xs.progress, Rect::new(1, 1, 59, 3), "three bare rows");
        assert!(!xs.queue_header());
        assert!(!regions(72, 17).unwrap().queue_header(), "below 18 rows");
        assert_eq!(regions(59, 16), None);
    }

    #[test]
    fn a_popup_covers_whole_panels() {
        let l = regions(200, 50).unwrap();
        assert_eq!(popup_area(Popup::Details, &l), l.right.unwrap());
        assert_eq!(popup_area(Popup::Wide, &l), l.body);
        let m = regions(120, 30).unwrap();
        assert_eq!(popup_area(Popup::Details, &m), m.body, "whole body at M");
        assert_eq!(popup_area(Popup::Account, &m), m.right.unwrap());
        assert_eq!(popup_area(Popup::Quit, &m), m.right.unwrap());
        assert_eq!(popup_area(Popup::SignIn, &m), m.body);
        let s = regions(80, 24).unwrap();
        assert_eq!(popup_area(Popup::Account, &s), s.body);
    }

    fn texts(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
        lines.iter().map(crate::tui::text::plain).collect()
    }

    #[test]
    fn a_popups_keys_go_in_its_border_when_they_all_fit() {
        let keys = [
            Hint::new("↑↓", "game", 1),
            Hint::new("o", "card page", 2),
            Hint::new("esc", "close", 0),
        ];
        let lines: Vec<_> = (0..3)
            .map(|i| ratatui::text::Line::from(format!("line {i}")))
            .collect();
        let placed = place(Rect::new(0, 0, 60, 10), &lines, &keys, 0).unwrap();
        assert_eq!(texts(&placed.lines), ["line 0", "line 1", "line 2"]);
        assert_eq!(
            placed.keys.as_ref().map(crate::tui::text::plain).as_deref(),
            Some(" ↑↓  game    o  card page    esc  close")
        );
        assert_eq!(placed.below, 0);
    }

    #[test]
    fn a_popup_that_scrolls_says_how_much_is_below() {
        let keys = [
            Hint::new("↑↓", "game", 1),
            Hint::new("o", "card page", 2),
            Hint::new("esc", "close", 0),
        ];
        let lines: Vec<_> = (0..30)
            .map(|i| ratatui::text::Line::from(format!("line {i}")))
            .collect();
        let placed = place(Rect::new(0, 0, 60, 13), &lines, &keys, 0).unwrap();
        assert_eq!(placed.lines.len(), 11);
        assert_eq!(placed.below, 19);
        assert_eq!(
            placed
                .keys
                .as_ref()
                .map(crate::tui::golden::line_text)
                .as_deref(),
            Some("19 more ↓ [PgDn]   [esc] close")
        );
    }

    #[test]
    fn keys_that_dont_fit_the_border_are_its_last_lines() {
        let keys = [
            Hint::new("enter", "sign in again", 0),
            Hint::new("d", "sign out", 0),
            Hint::new("v", "online", 1),
            Hint::new("esc", "close", 0),
        ];
        let lines = vec![ratatui::text::Line::from("› Steam  ● alice")];
        let placed = place(Rect::new(0, 0, 46, 21), &lines, &keys, 0).unwrap();
        assert_eq!(placed.keys, None);
        let last: Vec<String> = texts(&placed.lines[placed.lines.len() - 2..]);
        assert_eq!(
            last,
            [
                " enter  sign in again    d  sign out",
                " v  online    esc  close"
            ]
        );
        assert_eq!(placed.lines.len(), 19);
        assert!(place(Rect::new(0, 0, 10, 2), &lines, &keys, 0).is_err());
    }
}
