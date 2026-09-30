// The queue's ladders (docs/design/ui.md §2.3, §2.4): its column rule, its
// rows and section rules, the legend in its bottom border and how many games
// are out of sight.
//
// The column rule: the name keeps at least 24 columns, and optional columns
// join in this order while it does: ≈ DONE IN, pips, ≈ LEFT, ≈ A DROP,
// STATUS. Gaps are 2 when the queue is 100 columns or more inside, else 1.
// In a row they stand as STATUS, HOURS, DROPS, pips, ≈ A DROP, ≈ LEFT,
// ≈ DONE IN; where STATUS is gone, a set-aside game's ≈ DONE IN says so.

use chrono::{DateTime, FixedOffset, Utc};
use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::{
    super::{
        format,
        text::{Fits, Overflow, first_fit, fit, key, pips, rfit, rule, rule_ladder, shorten},
        theme,
    },
    NAME_MAX,
};
use crate::viewmodel::{Cell, Glyph, QueueEntry, RowStatus, Section, SectionDoing, SectionRows};

/// The name never has fewer than this.
const NAME_MIN: usize = 24;
const STATUS: usize = 10;
const HOURS: usize = 5;
const DROPS: usize = 5;
const MONEY: usize = 8;
const DONE_IN: usize = 9;

/// Which of the queue's columns show, and how wide the name is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueCols {
    pub(crate) inner: usize,
    pub(crate) gap: usize,
    pub(crate) name: usize,
    pub(crate) pip_width: usize,
    pub(crate) status: bool,
    pub(crate) pips: bool,
    pub(crate) per_drop: bool,
    pub(crate) left: bool,
    pub(crate) done_in: bool,
}

/// The queue's columns `inner` wide, the pips' column `pip_width`.
pub(crate) fn queue_cols(inner: usize, pip_width: usize) -> Fits<QueueCols> {
    let gap = if inner >= 100 { 2 } else { 1 };
    let mut fixed = 3 + 4 + (gap + HOURS) + (gap + DROPS);
    let mut c = QueueCols {
        inner,
        gap,
        name: 0,
        pip_width,
        status: false,
        pips: false,
        per_drop: false,
        left: false,
        done_in: false,
    };
    for (column, extra) in [
        ("done", gap + DONE_IN),
        ("pips", 1 + pip_width),
        ("left", gap + MONEY),
        ("per_drop", gap + MONEY),
        ("status", gap + STATUS),
    ] {
        if inner >= fixed + extra + NAME_MIN {
            fixed += extra;
            match column {
                "done" => c.done_in = true,
                "pips" => c.pips = true,
                "left" => c.left = true,
                "per_drop" => c.per_drop = true,
                _ => c.status = true,
            }
        }
    }
    c.name = NAME_MAX.min(inner.saturating_sub(fixed));
    if c.name < 12 {
        return Err(Overflow(format!(
            "the queue's name column is only {}",
            c.name
        )));
    }
    Ok(c)
}

/// The queue's column header: "#   GAME   STATUS   HOURS DROPS …".
pub(crate) fn queue_head(c: &QueueCols) -> Fits<Line<'static>> {
    let g = " ".repeat(c.gap);
    let mut out = format!(
        "   {}",
        fit(Line::from("#"), 4).map(|l| super::super::text::plain(&l))?
    );
    out.push_str(&super::super::text::plain(&fit(
        Line::from("GAME"),
        c.name,
    )?));
    if c.status {
        out.push_str(&format!("{g}{:<STATUS$}", "STATUS"));
    }
    out.push_str(&format!("{g}{:>HOURS$}{g}{:>DROPS$}", "HOURS", "DROPS"));
    if c.pips {
        out.push_str(&" ".repeat(1 + c.pip_width));
    }
    if c.per_drop {
        out.push_str(&format!("{g}{:>MONEY$}", "≈ A DROP"));
    }
    if c.left {
        out.push_str(&format!("{g}{:>MONEY$}", "≈ LEFT"));
    }
    if c.done_in {
        out.push_str(&format!("{g}{:>DONE_IN$}", "≈ DONE IN"));
    }
    Ok(Line::styled(out, theme::dim()))
}

/// A queue row's time: its ≈ DONE IN as time left, or with t as a clock
/// time; a done game's "✓ 17:44", or its day beyond today.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Times {
    pub(crate) clock: bool,
    pub(crate) now: DateTime<Utc>,
    pub(crate) zone: FixedOffset,
}

fn money_cell(cell: Option<Cell>) -> String {
    match cell {
        None => String::new(),
        Some(Cell::Value { value, .. }) => format::money(value),
        Some(Cell::Pending) => "…".to_owned(),
        Some(Cell::NoMarket | Cell::NotMarketable) => "—".to_owned(),
        Some(Cell::Failed) => "?".to_owned(),
        Some(Cell::Foreign(m)) => m.to_string(),
    }
}

fn status_words(status: RowStatus) -> (String, Style) {
    let needs = |h: f64| {
        if h > 0.0 {
            (format!("needs {h:.1}h"), theme::fg(theme::BUSY))
        } else {
            ("ready".to_owned(), theme::dim())
        }
    };
    match status {
        RowStatus::Farming => ("farming".to_owned(), theme::fg(theme::GOOD)),
        RowStatus::BuildingHours(h) | RowStatus::Needs(h) => needs(h),
        RowStatus::Waiting => ("waiting".to_owned(), theme::fg(theme::BUSY)),
        RowStatus::NextUp => ("next up".to_owned(), Style::new()),
        RowStatus::Ready => ("ready".to_owned(), theme::dim()),
        RowStatus::SetAside => ("set aside".to_owned(), theme::fg(theme::BUSY)),
        RowStatus::LeftAlone => ("left alone".to_owned(), theme::fg(theme::BUSY)),
        RowStatus::Skipped => ("skipped".to_owned(), theme::dim()),
        RowStatus::NotFarmed => ("not farmed".to_owned(), theme::dim()),
        RowStatus::Done => ("done".to_owned(), theme::dim()),
    }
}

/// A game's row, `c.inner` wide: the selection's ›, its glyph and rank, its
/// name, shortened at a word boundary if need be, and its columns.
pub(crate) fn queue_row(c: &QueueCols, e: &QueueEntry, t: Times) -> Fits<Line<'static>> {
    let g = || Span::raw(" ".repeat(c.gap));
    let muted = matches!(
        e.status,
        RowStatus::Skipped | RowStatus::Done | RowStatus::NotFarmed
    );
    let (glyph, glyph_style) = match e.glyph {
        Glyph::Farming => ("▶", theme::fg(theme::GOOD)),
        Glyph::Hours => ("▷", theme::fg(theme::GOOD)),
        Glyph::Waiting => ("‖", theme::fg(theme::BUSY)),
        Glyph::Done => ("✓", theme::fg(theme::GOOD)),
        Glyph::None => (" ", Style::new()),
    };
    let (tier, tier_style) = match (e.section(), e.tier) {
        (Section::Done, _) => (String::new(), Style::new()),
        (_, preferences::Tier::Priority(n)) => (format!("#{n}"), theme::strong(theme::BUSY)),
        (_, preferences::Tier::Skip) => ("✕".to_owned(), theme::fg(theme::BAD)),
        _ => (String::new(), Style::new()),
    };
    let name_style = if muted { theme::dim() } else { Style::new() };
    let mut spans = vec![
        Span::raw(if e.selected { "›" } else { " " }),
        Span::styled(glyph, glyph_style),
        Span::raw(" "),
    ];
    spans.extend(fit(Line::styled(tier, tier_style), 4)?.spans);
    spans.extend(
        fit(
            Line::styled(shorten(&e.game.name, c.name)?, name_style),
            c.name,
        )?
        .spans,
    );
    let (words, words_style) = status_words(e.status);
    if c.status {
        spans.push(g());
        spans.extend(fit(Line::styled(words.clone(), words_style), STATUS)?.spans);
    }
    let hours_style = if !muted && e.game.hours < 3.0 {
        theme::fg(theme::BUSY)
    } else if muted {
        theme::dim()
    } else {
        Style::new()
    };
    spans.push(g());
    spans.extend(
        rfit(
            Line::styled(format::hours(e.game.hours), hours_style),
            HOURS,
        )?
        .spans,
    );
    spans.push(g());
    let drops = format::slash(e.game.drops.received, e.game.drops.total());
    spans.extend(rfit(Line::styled(drops, name_style), DROPS)?.spans);
    if c.pips {
        spans.push(Span::raw(" "));
        let p = &e.pips;
        let shown = if (p.total as usize) <= c.pip_width {
            let mut l = pips(p.before, p.today, p.foils, p.total);
            if muted {
                l = l.style(theme::dim());
                for s in &mut l.spans {
                    s.style = theme::dim();
                }
            }
            l
        } else {
            Line::default()
        };
        spans.extend(fit(shown, c.pip_width)?.spans);
    }
    let money_style = if e.glyph == Glyph::Farming {
        Style::new()
    } else {
        theme::dim()
    };
    if c.per_drop {
        spans.push(g());
        spans.extend(rfit(Line::styled(money_cell(e.per_drop), money_style), MONEY)?.spans);
    }
    if c.left {
        spans.push(g());
        spans.extend(rfit(Line::styled(money_cell(e.left), money_style), MONEY)?.spans);
    }
    if c.done_in {
        let done = match (e.status, e.finished_at, e.done_in) {
            (RowStatus::SetAside | RowStatus::LeftAlone, _, _) if !c.status => words,
            (RowStatus::Skipped, _, _) => "never".to_owned(),
            (RowStatus::Done, Some(at), _) => {
                let when = format::clock(at, t.now, t.zone);
                if when.len() == 5 {
                    format!("✓ {when}")
                } else {
                    when
                }
            }
            (_, _, Some(d)) if t.clock => format::estimate_at(t.now, d, t.zone),
            (_, _, Some(d)) => format::estimate(d),
            _ => String::new(),
        };
        spans.push(g());
        spans.extend(rfit(Line::styled(done, money_style), DONE_IN)?.spans);
    }
    let row = Line::from(spans);
    if row.width() > c.inner {
        return Err(Overflow(format!(
            "a queue row is {} wide, over {}",
            row.width(),
            c.inner
        )));
    }
    Ok(row)
}

/// What a section's rule may say, besides its name and count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuleSays {
    /// What its hint says: its value, and what its tier means.
    pub(crate) value: bool,
    pub(crate) hint: bool,
    /// Done's games are shown (c).
    pub(crate) done_shown: bool,
}

fn title(name: &str, style: Style, rest: String) -> Line<'static> {
    Line::from(vec![Span::styled(name.to_owned(), style), Span::raw(rest)])
}

/// A section's rule, `w` wide: "── INDIFFERENT · 57 · farming ──── ≈ £15.34
/// left · after your priorities · closest to dropping first ─". The hint
/// gives up its longest part first, then the next, then the value.
pub(crate) fn section_rule(rows: &SectionRows, w: usize, says: RuleSays) -> Fits<Line<'static>> {
    let n = rows.entries.len();
    let doing = match rows.doing {
        Some(SectionDoing::Farming) => " · farming",
        Some(SectionDoing::BuildingHours) => " · building hours",
        Some(SectionDoing::Waiting) => " · waiting",
        None => "",
    };
    let value_hints = |tail: &[&str]| -> Vec<String> {
        let Some(left) = rows.left.filter(|_| says.value) else {
            return Vec::new();
        };
        if left.unpriced_games > 0 {
            return vec![
                format!(
                    "{} left so far · {} not priced yet",
                    format::about(left.value),
                    left.unpriced_games
                ),
                format::so_far(&left),
            ];
        }
        let v = format!("{} left", format::about(left.value));
        let mut out: Vec<String> = tail.iter().map(|t| format!("{v} · {t}")).collect();
        out.push(v);
        out
    };
    let lines = |hints: Vec<String>| hints.into_iter().map(Line::from).collect::<Vec<_>>();
    match rows.section {
        Section::Priority if n == 0 => {
            let options = [
                vec![
                    Span::raw(" · nothing ranked yet: select a game and press "),
                    key("1"),
                    Span::raw(" to farm it first"),
                ],
                vec![
                    Span::raw(" · press "),
                    key("1"),
                    Span::raw(" on a game to farm it first"),
                ],
                vec![Span::raw(" · press "), key("1"), Span::raw(" on a game")],
                vec![Span::raw(" · "), key("1"), Span::raw(" ranks a game")],
                vec![],
            ];
            let heading = Span::styled("PRIORITY", theme::strong(theme::BUSY));
            first_fit(
                options.into_iter().filter_map(|rest| {
                    let mut t = vec![heading.clone(), Span::raw(" · 0")];
                    t.extend(rest);
                    rule(Line::from(t), w, None).ok()
                }),
                w,
            )
        }
        Section::Priority => {
            let mut hints = value_hints(&["farmed first, in your order"]);
            if says.hint {
                hints.push("farmed first, in your order".to_owned());
            }
            rule_ladder(
                title(
                    "PRIORITY",
                    theme::strong(theme::BUSY),
                    format!(" · {n}{doing}"),
                ),
                w,
                dim_all(lines(hints)),
            )
        }
        Section::Indifferent => {
            let mut hints = if n == 0 {
                vec!["nothing left to farm".to_owned()]
            } else {
                value_hints(&[
                    "after your priorities · closest to dropping first",
                    "closest to dropping first",
                ])
            };
            if says.hint && n > 0 {
                hints.extend([
                    "after your priorities · closest to dropping first".to_owned(),
                    "closest to dropping first".to_owned(),
                ]);
            }
            rule_ladder(
                title("INDIFFERENT", theme::bold(), format!(" · {n}{doing}")),
                w,
                dim_all(lines(hints)),
            )
        }
        Section::Skipped => rule_ladder(
            title("SKIPPED", theme::strong(theme::BAD), format!(" · {n}")),
            w,
            dim_all(lines(vec!["never farmed".to_owned()])),
        ),
        Section::Done => {
            let count = match (n, rows.this_session) {
                (0, _) => " · none yet".to_owned(),
                (n, k) if k == n => format!(" · {n} this session"),
                (n, k) => format!(" · {n} · {k} this session"),
            };
            let (count, action) = if says.done_shown {
                (format!("{count}, newest first"), " to hide")
            } else {
                (count, " to show")
            };
            let hint = Line::from(vec![key("c"), Span::styled(action, theme::dim())]);
            let hints = if n == 0 { Vec::new() } else { vec![hint] };
            rule_ladder(title("DONE", theme::strong(theme::GOOD), count), w, hints)
        }
    }
}

fn dim_all(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    lines.into_iter().map(|l| l.style(theme::dim())).collect()
}

/// The legend in the queue's bottom border, as much as fits in `w`.
pub(crate) fn legend(w: usize) -> Line<'static> {
    let options = [
        "▶ farming  ▷ building hours  ‖ waiting  #1 priority  ✕ skipped   ● had  ◆ this session  ★ foil  ○ to come",
        "▶ farming  ▷ hours  ‖ waiting  ✕ skipped   ● had  ◆ this session  ★ foil  ○ to come",
        "▶ farming  ▷ hours  ✕ skipped  ● had ◆ today ★ foil ○ to come",
        "● had ◆ this session ★ foil ○ to come",
        "● had ◆ today ○ to come",
        "",
    ];
    first_fit(options.map(|o| Line::styled(o, theme::dim())), w).unwrap_or_default()
}

/// How many games are out of sight: "42 more ↓", "47 more ↑", counting
/// games, never rows.
pub(crate) fn more(above: usize, below: usize) -> String {
    let mut parts = Vec::new();
    if above > 0 {
        parts.push(format!("{above} more ↑"));
    }
    if below > 0 {
        parts.push(format!("{below} more ↓"));
    }
    parts.join(" · ")
}

/// A line of the queue: a section's rule, or a game's row.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Row {
    Rule(Line<'static>),
    Game(u32, Line<'static>),
}

/// The rows that show from `offset`, `room` of them, and how many games
/// are above and below them.
pub(crate) fn window(rows: &[Row], room: usize, offset: usize) -> (&[Row], usize, usize) {
    let offset = offset.min(rows.len().saturating_sub(room));
    let end = (offset + room).min(rows.len());
    let games = |rows: &[Row]| rows.iter().filter(|r| matches!(r, Row::Game(..))).count();
    (
        &rows[offset..end],
        games(&rows[..offset]),
        games(&rows[end..]),
    )
}

/// The queue's title, in its top border, longest first: what's to go, with
/// the time to finish at L.
pub(crate) fn queue_title(
    games: usize,
    drops: u32,
    eta: Option<std::time::Duration>,
) -> Vec<Line<'static>> {
    if games == 0 {
        return vec![Line::styled("nothing to go", theme::dim()), Line::default()];
    }
    let to_go = format!("{games} to go · {}", format::drops(drops));
    let mut out = Vec::new();
    if let Some(eta) = eta {
        out.push(format!("{to_go} · {} to finish", format::eta(eta)));
    }
    out.push(to_go);
    out.push(format!("{games} to go"));
    out.into_iter()
        .map(|t| Line::styled(t, theme::dim()))
        .collect()
}

/// Every row of the queue, in order, as rules and games. Done's games show
/// only when asked for (c).
pub(crate) fn rows(
    q: &crate::viewmodel::Queue,
    c: &QueueCols,
    t: Times,
    says: RuleSays,
) -> Fits<Vec<Row>> {
    let mut out = Vec::new();
    for section in &q.sections {
        if section.section == Section::Skipped && section.entries.is_empty() {
            continue;
        }
        out.push(Row::Rule(section_rule(section, c.inner, says)?));
        if section.section == Section::Done && !says.done_shown {
            continue;
        }
        for e in &section.entries {
            out.push(Row::Game(e.game.app_id, queue_row(c, e, t)?));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tui::golden::{line_text, mockup},
        viewmodel::{Queue, fixtures},
    };

    fn times(data: &fixtures::DataSet) -> Times {
        Times {
            clock: false,
            now: data.now,
            zone: fixtures::zone(),
        }
    }

    /// A mockup's rows inside the queue panel: from `x`, `w` wide.
    fn inside(title: &str, rows: std::ops::Range<usize>, x: usize, w: usize) -> Vec<String> {
        mockup(title).rows[rows]
            .iter()
            .map(|r| {
                r.chars()
                    .skip(x)
                    .take(w)
                    .collect::<String>()
                    .trim_end()
                    .to_owned()
            })
            .collect()
    }

    fn texts(rows: &[Row]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                Row::Rule(l) | Row::Game(_, l) => line_text(l).trim_end().to_owned(),
            })
            .collect()
    }

    #[test]
    fn the_column_sets_the_spec_gives_each_size() {
        let set = |inner| {
            let c = queue_cols(inner, 15).unwrap();
            let mut have = Vec::new();
            for (on, name) in [
                (c.status, "STATUS"),
                (true, "HOURS"),
                (true, "DROPS"),
                (c.pips, "pips"),
                (c.per_drop, "≈ A DROP"),
                (c.left, "≈ LEFT"),
                (c.done_in, "≈ DONE IN"),
            ] {
                if on {
                    have.push(name);
                }
            }
            (have.join(", "), c.name, c.gap)
        };
        let all = "STATUS, HOURS, DROPS, pips, ≈ A DROP, ≈ LEFT, ≈ DONE IN";
        assert_eq!(set(129), (all.to_owned(), 49, 2), "209 × 49");
        assert_eq!(set(120), (all.to_owned(), 40, 2), "200 × 50");
        assert_eq!(
            set(83),
            ("HOURS, DROPS, pips, ≈ LEFT, ≈ DONE IN".to_owned(), 29, 1),
            "146 × 40"
        );
        assert_eq!(
            set(70),
            ("HOURS, DROPS, pips, ≈ DONE IN".to_owned(), 25, 1),
            "120 × 30"
        );
        assert_eq!(
            set(56),
            ("HOURS, DROPS, ≈ DONE IN".to_owned(), 27, 1),
            "100 × 26"
        );
        assert_eq!(
            set(76),
            ("HOURS, DROPS, pips, ≈ DONE IN".to_owned(), 31, 1),
            "80 × 24"
        );
        assert_eq!(set(56).0, "HOURS, DROPS, ≈ DONE IN", "60 × 16");
        assert!(queue_cols(20, 15).is_err());
    }

    #[test]
    fn the_queue_at_l_is_the_spec_s() {
        let data = fixtures::farming_alone();
        let q = Queue::build(&data.snapshot(), Some(fixtures::HEAVY_RAIN));
        let c = queue_cols(120, usize::try_from(q.pip_width).unwrap()).unwrap();
        let says = RuleSays {
            value: true,
            hint: true,
            done_shown: false,
        };
        let all = rows(&q, &c, times(&data), says).unwrap();
        let (shown, above, below) = window(&all, 37, 0);
        assert_eq!((above, below), (0, 23), "23 more ↓");
        let mut got = vec![line_text(&queue_head(&c).unwrap()).trim_end().to_owned()];
        got.extend(texts(shown));
        assert_eq!(got, inside("dashboard, farming alone (L)", 9..47, 2, 120));
    }

    #[test]
    fn the_queue_at_m_s_and_xs_is_the_spec_s() {
        let data = fixtures::farming_alone();
        let q = Queue::build(&data.snapshot(), Some(fixtures::HEAVY_RAIN));
        let pw = usize::try_from(q.pip_width).unwrap();
        let says = RuleSays {
            value: true,
            hint: true,
            done_shown: false,
        };
        for (title, inner, first, count) in [
            ("dashboard, farming alone (M)", 70, 8, 19),
            ("dashboard, farming alone (M, tall)", 83, 9, 28),
            ("dashboard, farming alone (S)", 76, 7, 14),
        ] {
            let c = queue_cols(inner, pw).unwrap();
            let all = rows(&q, &c, times(&data), says).unwrap();
            let (shown, _, _) = window(&all, count - 1, 0);
            let mut got = vec![line_text(&queue_head(&c).unwrap()).trim_end().to_owned()];
            got.extend(texts(shown));
            assert_eq!(
                got,
                inside(title, first..first + count, 2, inner),
                "{title}"
            );
        }
        let c = queue_cols(56, pw).unwrap();
        let bare = RuleSays {
            value: false,
            hint: false,
            done_shown: false,
        };
        let all = rows(&q, &c, times(&data), bare).unwrap();
        let (shown, _, below) = window(&all, 8, 0);
        assert_eq!(
            texts(shown),
            inside("dashboard, farming alone (XS)", 5..13, 2, 56)
        );
        assert_eq!(more(0, below), "52 more ↓");
    }

    #[test]
    fn the_queue_scrolled_to_its_end() {
        let data = fixtures::queue_at_its_end();
        let q = Queue::build(&data.snapshot(), Some(fixtures::WARFRAME));
        let c = queue_cols(76, usize::try_from(q.pip_width).unwrap()).unwrap();
        let says = RuleSays {
            value: true,
            hint: true,
            done_shown: false,
        };
        let all = rows(&q, &c, times(&data), says).unwrap();
        let (shown, above, below) = window(&all, 13, usize::MAX);
        assert_eq!((above, below), (47, 0), "47 more ↑");
        assert_eq!(
            texts(shown),
            inside("the queue scrolled to its end (S)", 8..21, 2, 76)
        );
    }

    #[test]
    fn done_in_can_be_clock_times() {
        let data = fixtures::farming_alone();
        let q = Queue::build(&data.snapshot(), None);
        let c = queue_cols(70, 15).unwrap();
        let t = Times {
            clock: true,
            ..times(&data)
        };
        let row = |id| line_text(&queue_row(&c, q.get(id).unwrap(), t).unwrap());
        assert!(
            row(fixtures::HEAVY_RAIN).ends_with("17:55"),
            "{}",
            row(fixtures::HEAVY_RAIN)
        );
        assert!(row(fixtures::LIMBO).ends_with("19:00"));
    }

    #[test]
    fn each_sections_rule_gives_up_its_hints_in_order() {
        let data = fixtures::first_minutes();
        let q = Queue::build(&data.snapshot(), None);
        let says = RuleSays {
            value: true,
            hint: true,
            done_shown: false,
        };
        let rule = section_rule(q.section(Section::Indifferent), 70, says).unwrap();
        assert_eq!(
            line_text(&rule),
            "── INDIFFERENT · 62 · farming ───────── ≈ £2.37 so far · 48 unpriced ─"
        );
        let data = fixtures::building_hours();
        let q = Queue::build(&data.snapshot(), None);
        let rule = section_rule(q.section(Section::Priority), 70, says).unwrap();
        assert_eq!(
            line_text(&rule),
            "── PRIORITY · 2 · building hours ────────────────────── ≈ £0.26 left ─"
        );
    }

    #[test]
    fn done_says_newest_first_when_shown() {
        let data = fixtures::nothing_to_farm();
        let q = Queue::build(&data.snapshot(), Some(fixtures::VAMPIRE_SURVIVORS));
        let says = RuleSays {
            value: false,
            hint: true,
            done_shown: true,
        };
        assert_eq!(
            line_text(&section_rule(q.section(Section::Done), 70, says).unwrap()),
            "── DONE · 62 this session, newest first ──────────────── [c] to hide ─"
        );
        assert_eq!(
            line_text(&section_rule(q.section(Section::Indifferent), 70, says).unwrap()),
            "── INDIFFERENT · 0 ──────────────────────────── nothing left to farm ─"
        );
    }

    #[test]
    fn the_legend_and_the_title_give_way() {
        assert_eq!(
            line_text(&legend(124 - 6 - 9 - 4)),
            "▶ farming  ▷ building hours  ‖ waiting  #1 priority  ✕ skipped   ● had  ◆ this session  ★ foil  ○ to come"
        );
        assert_eq!(
            line_text(&legend(40)),
            "● had ◆ this session ★ foil ○ to come"
        );
        assert_eq!(line_text(&legend(3)), "");
        let eta = Some(std::time::Duration::from_secs((4 * 24 + 21) * 3600));
        let titles: Vec<String> = queue_title(57, 236, eta).iter().map(line_text).collect();
        assert_eq!(
            titles,
            [
                "57 to go · 236 drops · ≈ 4d 21h to finish",
                "57 to go · 236 drops",
                "57 to go"
            ]
        );
        assert_eq!(line_text(&queue_title(0, 0, None)[0]), "nothing to go");
        assert_eq!(more(47, 0), "47 more ↑");
        assert_eq!(more(3, 9), "3 more ↑ · 9 more ↓");
    }
}
