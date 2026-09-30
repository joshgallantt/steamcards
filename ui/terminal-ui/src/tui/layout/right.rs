// The right-hand column's ladders (docs/design/ui.md §2.1, §2.4): the Now
// panel at L; the chosen game's forms, richest first (the set as a table,
// two cards a row, one a row, then one line), and how the column is shared
// with this session's cards at M; and the haul's rows at L and at M.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::{
    super::{
        format,
        text::{
            Fits, first_fit, fit, gauge, glue, join, key, rfit, rule, rule_ladder, shorten, spread,
            wrap,
        },
        theme,
    },
    progress,
};
use crate::viewmodel::{Cell, ChosenGame, Doing, Haul, HaulRow, Now, Progress, TheSet, Told};
use preferences::Tier;

fn raw(s: impl Into<String>) -> Span<'static> {
    Span::raw(s.into())
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

fn plain(s: impl Into<String>) -> Line<'static> {
    Line::from(s.into())
}

/// A card, as a row says it: "Madison", "★ Thanatos (foil)", "Zagreus,
/// 2nd copy", "⠋ finding out which card", in at most `w` columns.
pub(crate) fn card_words(card: &Told, copy: Option<u32>, spinner: &str, w: usize) -> Line<'static> {
    match card {
        Told::Identifying => first_fit(
            [
                Line::from(vec![
                    Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                    dim(" finding out which card"),
                ]),
                Line::from(vec![
                    Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                    dim(" identifying"),
                ]),
            ],
            w,
        )
        .unwrap_or_default(),
        Told::Unknown => first_fit(
            [
                plain("couldn't tell which card").style(theme::dim()),
                plain("unknown").style(theme::dim()),
            ],
            w,
        )
        .unwrap_or_default(),
        Told::Named { name, foil } => {
            let mut spans = Vec::new();
            if *foil {
                spans.push(Span::styled("★ ", theme::fg(theme::ACCENT)));
            }
            spans.push(raw(name.clone()));
            if *foil && w >= 17 {
                spans.push(raw(" (foil)"));
            }
            let used: usize = spans.iter().map(Span::width).sum();
            let copy_words = match copy {
                Some(1) => None,
                Some(n) => Some((
                    format!(", {} copy", ordinal(n)),
                    format!(" ({})", ordinal(n)),
                )),
                None => Some((", copy ?".to_owned(), " (?)".to_owned())),
            };
            if let Some((long, short)) = copy_words
                && used + 6 <= w
            {
                let room = w - used;
                if let Ok(l) = first_fit([Line::from(long), Line::from(short)], room) {
                    spans.push(dim(super::super::text::plain(&l)));
                }
            }
            Line::from(spans)
        }
    }
}

/// "2nd", "3rd".
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// A price as a cell says it: "£0.05", "£0.06 8h" once stale, "…" on its
/// way, "no market", "—", "?", or another currency's amount as it came.
pub(crate) fn price_words(cell: Option<Cell>) -> Line<'static> {
    match cell {
        Some(Cell::Value { value, stale: None }) => plain(format::money(value)),
        Some(Cell::Value {
            value,
            stale: Some(age),
        }) => Line::styled(
            format!("{} {}", format::money(value), format::age(age)),
            theme::dim(),
        ),
        Some(Cell::Pending) | None => Line::styled("…", theme::dim()),
        Some(Cell::NoMarket) => Line::styled("no market", theme::dim()),
        Some(Cell::NotMarketable) => Line::styled("—", theme::dim()),
        Some(Cell::Failed) => Line::styled("?", theme::fg(theme::BUSY)),
        Some(Cell::Foreign(m)) => plain(m.to_string()),
    }
}

/// A price in the chosen game's set: as `price_words` says it, but a stale
/// one without its age, which the set's rule says for all of them ("8h
/// old"), so the prices under it are never only dimmed.
fn set_price_words(cell: Cell) -> Line<'static> {
    match cell {
        Cell::Value {
            value,
            stale: Some(_),
        } => Line::styled(format::money(value), theme::dim()),
        other => price_words(Some(other)),
    }
}

// ── Now, at L ────────────────────────────────────────────────────────────────

/// The Now panel's inside at L, `w` wide, and what its borders say: since
/// when (top right) and how often it looks (bottom left).
pub(crate) fn now_panel(
    p: &Progress,
    now: &Now,
    spinner: &str,
    w: usize,
) -> Fits<(Vec<Line<'static>>, Option<String>, Option<String>)> {
    const LABEL: usize = 12;
    let label = |t: &str| fit(plain(t), LABEL);
    let indent = |l: Line<'static>| join([plain("  "), l]);
    let good = theme::fg(theme::GOOD);
    if let Some(group) = &now.group {
        let mut rows = vec![
            Line::from(vec![
                Span::styled("▷", good),
                raw(format!(
                    " Building hours on {} games together",
                    group.games.len()
                )),
            ]),
            plain("  Cards drop once a game has 3 hours."),
        ];
        for g in group.games.iter().take(3) {
            let rank = g.rank.map_or_else(String::new, |n| format!("#{n}"));
            rows.push(join([
                plain("  "),
                fit(Line::styled(rank, theme::strong(theme::BUSY)), 3)?,
                fit(plain(shorten(&g.name, 18)?), 18)?,
                plain(format!(" {} ", format::hours(g.hours))),
                gauge((g.hours * 10.0) as u32, 30, 12, theme::fg(theme::LINK)),
                plain(format!("  {:.1}h to go", g.to_go)),
            ]));
        }
        if let Some(last) = group.games.last().filter(|_| group.games.len() > 3) {
            rows.push(plain(format!(
                "  {} more, down to {} at {}",
                group.games.len() - 3,
                last.name,
                format::hours(last.hours)
            )));
        }
        let since = group
            .since
            .map(|t| format!("since {}", format::clock(t, p.now, p.zone)));
        return Ok((rows, since, None));
    }
    let Some(g) = &now.game else {
        let rows = match message_rows(p, now, spinner, w) {
            Some(rows) => rows?,
            None => Vec::new(),
        };
        return Ok((rows, None, None));
    };
    let look = now.next_look.map_or_else(String::new, |t| {
        format!("next look {}", format::countdown(t, p.now))
    });
    let pips = super::super::text::pips(g.pips.before, g.pips.today, g.pips.foils, g.pips.total);
    let mut spaced: Vec<Span<'static>> = Vec::new();
    for (i, span) in pips.spans.iter().enumerate() {
        for (j, ch) in span.content.chars().enumerate() {
            if i > 0 || j > 0 {
                spaced.push(raw("  "));
            }
            spaced.push(Span::styled(ch.to_string(), span.style));
        }
    }
    let on_it = g.since.map_or_else(String::new, |t| {
        format!(
            " · on it {}",
            format::duration((p.now - t).to_std().unwrap_or_default())
        )
    });
    let mut counts = spaced;
    counts.insert(0, raw("  "));
    counts.push(raw(format!(
        "    {} drops · {}{on_it}",
        format::of(g.received, g.total),
        format::to_go(g.remaining)
    )));
    let mut rows = vec![
        spread(
            Line::from(vec![Span::styled("▶ Farming ", good), raw(g.name.clone())]),
            plain(look),
            w,
            1,
        )?,
        Line::from(counts),
    ];
    let at = |t: chrono::DateTime<chrono::Utc>| {
        format::estimate_at(p.now, (t - p.now).to_std().unwrap_or_default(), p.zone)
    };
    if let Some(first) = g.first_card {
        rows.push(indent(join([
            label("First card")?,
            plain(format!("≈ {}, if a card drops every 30 min", at(first))),
        ])));
    } else if let Some(last) = &g.last_drop {
        let ago = format::duration((p.now - last.at).to_std().unwrap_or_default());
        let card = card_words(&last.card, None, spinner, w);
        rows.push(indent(join([
            label("Last drop")?,
            plain(format!(
                "{ago} ago, at {} · ",
                format::clock(last.at, p.now, p.zone)
            )),
            card,
        ])));
    }
    let next = &g.next_card;
    if next.cards > 0 {
        let mut text = format!(
            "one of its {} cards, {} of them new to you",
            next.cards, next.new
        );
        if let Some((lo, hi)) = next.normal {
            text.push_str(&format!(
                ": {}",
                glue(&format!("{} – {}", format::money(lo), format::money(hi)))
            ));
        }
        if let Some((lo, hi)) = next.foil {
            text.push_str(&format!(
                ", or rarely a foil, {}",
                glue(&format!("{} – {}", format::money(lo), format::money(hi)))
            ));
        }
        let wrapped = wrap(text, w - 2 - LABEL, 0)?;
        for (i, l) in wrapped.into_iter().enumerate() {
            let head = if i == 0 {
                label("Next card")?
            } else {
                plain(" ".repeat(LABEL))
            };
            rows.push(indent(join([head, l])));
        }
    }
    if let Some(last) = g.last_card {
        let then = g
            .then
            .as_ref()
            .map_or_else(String::new, |t| format!(" · then {t}"));
        let about = format::estimate((last - p.now).to_std().unwrap_or_default());
        let words = if g.first_card.is_some() {
            format!("≈ {}, the same way", at(last))
        } else {
            format!("≈ {}, in about {about}{then}", at(last))
        };
        rows.push(indent(join([label("Last card")?, plain(words)])));
    }
    let since = g
        .since
        .map(|t| format!("since {}", format::clock(t, p.now, p.zone)));
    let every = now.look_every.map(|d| {
        let minutes = d.as_secs() / 60;
        if g.remaining == 1 {
            format!("looks every {minutes} min: it's the last card")
        } else {
            format!("looks every {minutes} min")
        }
    });
    Ok((rows, since, every))
}

/// What a state other than farming says in the Now panel: its sentence and
/// what happens next, wrapped.
fn message_rows(
    p: &Progress,
    now: &Now,
    spinner: &str,
    w: usize,
) -> Option<Fits<Vec<Line<'static>>>> {
    let (sentence, next) = progress::sentence(p, now, spinner)?;
    Some((|| {
        let mut rows = wrap(sentence, w, 0)?;
        if let Some(next) = next {
            for l in wrap(next, w.saturating_sub(2), 0)? {
                rows.push(join([plain("  "), l]));
            }
        }
        Ok(rows)
    })())
}

// ── The chosen game ──────────────────────────────────────────────────────────

/// The chosen game's panel, `w` wide inside, richest form first: the set as
/// a table with its short-of-a-badge line, the table without it, two cards
/// a row, one a row, and the set in one line. Its farm priority is in every
/// form.
pub(crate) fn chosen_forms(g: &ChosenGame, w: usize) -> Fits<Vec<Vec<Line<'static>>>> {
    let status = first_fit(status_lines(g), w)?;
    let facts = first_fit(facts_lines(g), w)?;
    let mut prio = vec![rule(
        Line::styled("Farm priority", theme::heading()),
        w,
        None,
    )?];
    prio.extend(radio_rows(g, w)?);
    let age = g.stale.map(format::age);
    let hint_long: Vec<Line<'static>> = match &age {
        Some(a) => vec![
            plain(format!("list prices · {a} old")),
            plain(format!("{a} old")),
        ],
        None => vec![plain("list prices")],
    };
    let hint_short: Vec<Line<'static>> = match &age {
        Some(a) => vec![plain(format!("list · {a} old")), plain(format!("{a} old"))],
        None => vec![plain("list")],
    };
    let dim_all = |v: Vec<Line<'static>>| -> Vec<Line<'static>> {
        v.into_iter().map(|l| l.style(theme::dim())).collect()
    };
    let Some(set) = &g.set else {
        let range = |r: Option<(market::Money, market::Money)>| {
            r.map(|(a, b)| format!("{} – {}", format::money(a), format::money(b)))
        };
        let mut body = vec![rule_ladder(
            Line::styled("The set · not read yet", theme::heading()),
            w,
            dim_all(vec![plain("list prices")]),
        )?];
        let cards = range(g.normal_range).unwrap_or_else(|| "not priced yet".to_owned());
        let options = match range(g.foil_range) {
            Some(foils) => vec![
                format!("cards {cards} · foils {foils}"),
                format!("cards {cards}"),
            ],
            None => vec![format!("cards {cards}")],
        };
        body.push(first_fit(options, w)?);
        body.push(first_fit(
            [
                Line::from(vec![key("enter"), raw(" reads its card page, for the set")]),
                Line::from(vec![key("enter"), raw(" reads it")]),
            ],
            w,
        )?);
        let mut form = vec![status, facts];
        form.extend(body);
        form.extend(prio);
        return Ok(vec![form]);
    };
    let size = set.cards.len();
    let spares = if set.spares > 0 {
        format!(" · {}", format::spares(set.spares))
    } else {
        String::new()
    };
    let title = format!("The set · {}", format::the_set(set.have, size, set.spares));
    let short_title = format!("Set · {} of {size}{spares}", set.have);
    let mut forms = Vec::new();
    if let Ok(table) = table_rows(g, set, w) {
        let short = short_line(set, w.saturating_sub(3)).map(|l| join([plain("   "), l]));
        let head = rule_ladder(
            Line::styled(title.clone(), theme::heading()),
            w,
            dim_all(hint_long.clone()),
        )?;
        if let Some(short) = short.ok().filter(|l| l.width() <= w) {
            let mut full = vec![status.clone(), facts.clone(), head.clone()];
            full.extend(table.clone());
            full.push(short);
            full.extend(prio.clone());
            forms.push(full);
        }
        let mut plain_table = vec![status.clone(), facts.clone(), head];
        plain_table.extend(table);
        plain_table.extend(prio.clone());
        forms.push(plain_table);
    }
    let set_head = rule_ladder(
        Line::styled(short_title.clone(), theme::heading()),
        w,
        dim_all(hint_short),
    )?;
    if let Ok(two) = two_a_row(set, w) {
        let mut form = vec![status.clone(), facts.clone(), set_head.clone()];
        form.extend(two);
        form.extend(prio.clone());
        forms.push(form);
    }
    if let Ok(one) = one_a_row(set, w) {
        let mut form = vec![status.clone(), facts.clone(), set_head];
        form.extend(one);
        form.extend(prio.clone());
        forms.push(form);
    }
    let one_line = first_fit(
        [
            Line::from(vec![
                raw(format!("Set: {} of {size} cards{spares} · ", set.have)),
                key("enter"),
                raw(" for each"),
            ]),
            Line::from(vec![
                raw(format!("Set: {} of {size}{spares} · ", set.have)),
                key("enter"),
            ]),
            plain(format!("Set: {} of {size}", set.have)),
        ],
        w,
    )?;
    let mut tiny = vec![status, facts, one_line];
    tiny.extend(prio);
    forms.push(tiny);
    Ok(forms)
}

fn status_lines(g: &ChosenGame) -> Vec<Line<'static>> {
    let good = theme::fg(theme::GOOD);
    let (doing, style) = match g.doing {
        Doing::Farming => ("▶ farming now".to_owned(), good),
        Doing::BuildingHours { .. } => ("▷ building hours".to_owned(), good),
        Doing::Waiting => ("‖ waiting".to_owned(), theme::fg(theme::BUSY)),
        Doing::NextUp => ("next up".to_owned(), Style::new()),
        Doing::Queued => ("queued".to_owned(), Style::new()),
        Doing::SetAside(_) => ("set aside".to_owned(), theme::fg(theme::BUSY)),
        Doing::Skipped => ("✕ skipped".to_owned(), theme::fg(theme::BAD)),
        Doing::NotFarmed => ("not farmed".to_owned(), theme::dim()),
        Doing::Done { at: Some(at) } => (
            format!("✓ done at {}", format::clock(at, g.now, g.zone)),
            good,
        ),
        Doing::Done { at: None } => ("✓ done".to_owned(), good),
    };
    let badge = match g.badge_level {
        0 => "no badge yet".to_owned(),
        n => format!("badge level {n}"),
    };
    vec![
        Line::from(vec![
            dim(format!("App {} · {badge} · ", g.app_id)),
            Span::styled(doing.clone(), style),
        ]),
        Line::from(vec![
            Span::styled(doing.clone(), style),
            dim(format!(" · App {}", g.app_id)),
        ]),
        Line::from(vec![Span::styled(doing, style)]),
    ]
}

fn facts_lines(g: &ChosenGame) -> Vec<Line<'static>> {
    let hours = format::hours(g.hours);
    let drops = format!("{} drops", format::of(g.received, g.total));
    if g.hours_to_go > 0.0 && g.remaining > 0 {
        let need = format!("{:.1}h", g.hours_to_go);
        return vec![
            plain(format!("{hours} on record, needs {need} more · {drops}")),
            plain(format!("{hours}, needs {need} · {drops}")),
            plain(format!("{hours} · {drops}")),
        ];
    }
    let each = match g.per_drop {
        Some(Cell::Value { value, .. }) if g.remaining > 0 => {
            format!(" · ≈ {} a drop", format::money(value))
        }
        _ => String::new(),
    };
    let left = if g.remaining > 0 {
        format!(" · {}", format::to_go(g.remaining))
    } else {
        String::new()
    };
    let all = if g.remaining > 0 {
        left.clone()
    } else {
        " · all dropped".to_owned()
    };
    vec![
        plain(format!("{hours} on record · {drops}{all}{each}")),
        plain(format!("{hours} · {drops}{left}{each}")),
        plain(format!("{hours} · {drops}{left}")),
        plain(format!("{hours} · {drops}")),
    ]
}

/// The farm priority as radio buttons, each with its key, described as
/// fully as the width allows: all three the same way.
pub(crate) fn radio_rows(g: &ChosenGame, w: usize) -> Fits<Vec<Line<'static>>> {
    let priority = match g.tier {
        Tier::Priority(n) => format!("Priority #{n}"),
        _ => "Priority".to_owned(),
    };
    let on = |yes: bool, style: Style| {
        if yes {
            Span::styled(" ◉ ", style)
        } else {
            dim(" ○ ")
        }
    };
    let options = [
        (
            on(
                matches!(g.tier, Tier::Priority(_)),
                theme::strong(theme::BUSY),
            ),
            priority,
            "1-9",
        ),
        (
            on(g.tier == Tier::Indifferent, theme::bold()),
            "Indifferent".to_owned(),
            " 0 ",
        ),
        (
            on(g.tier == Tier::Skip, theme::strong(theme::BAD)),
            "Skip".to_owned(),
            " x ",
        ),
    ];
    let sets: [[&str; 3]; 3] = [
        [
            "farmed first, in rank order",
            "after your priorities",
            "never farmed",
        ],
        ["farmed first", "after priorities", "never farmed"],
        ["", "", ""],
    ];
    for words in sets {
        let rows: Vec<Line<'static>> = options
            .iter()
            .zip(words)
            .map(|((mark, label, k), what)| {
                let mut spans = vec![mark.clone()];
                spans.extend(fit(plain(label.clone()), 13).unwrap_or_default().spans);
                spans.push(key(k));
                if !what.is_empty() {
                    spans.push(dim(format!("  {what}")));
                }
                Line::from(spans)
            })
            .collect();
        if rows.iter().all(|r| r.width() <= w) {
            return Ok(rows);
        }
    }
    Err(super::super::text::Overflow(format!(
        "the farm priority over {w}"
    )))
}

fn count_words(owned: u32) -> Line<'static> {
    if owned == 0 {
        Line::styled("—", theme::dim())
    } else {
        plain(format::count(owned))
    }
}

/// How many columns the widest of `cells` takes.
fn widest(cells: impl Iterator<Item = Line<'static>>) -> usize {
    cells.map(|l| l.width()).max().unwrap_or(0)
}

/// The set as a table: CARD, OWNED, NORMAL, FOIL, and a mark for a card
/// that dropped today where there's room for it. A card's name is never
/// shortened, and a price's column is as wide as its widest ("no market"),
/// two columns clear of the one before.
fn table_rows(g: &ChosenGame, set: &TheSet, w: usize) -> Fits<Vec<Line<'static>>> {
    let name_w = widest(set.cards.iter().map(|c| plain(c.name.clone()))).max(14);
    let normal_w = (widest(set.cards.iter().map(|c| set_price_words(c.normal))) + 2).max(9);
    let foil_w = (widest(set.cards.iter().map(|c| set_price_words(c.foil))) + 2).max(8);
    let head = Line::styled(
        format!(
            "   {:<name_w$}{:>6}{:>normal_w$}{:>foil_w$}",
            "CARD", "OWNED", "NORMAL", "FOIL"
        ),
        theme::dim(),
    );
    let mut rows = vec![head];
    for c in &set.cards {
        let mut row = join([
            plain("   "),
            fit(plain(c.name.clone()), name_w)?,
            rfit(count_words(c.owned), 6)?,
            rfit(set_price_words(c.normal), normal_w)?,
            rfit(set_price_words(c.foil), foil_w)?,
        ]);
        if let Some(first) = c.today.first() {
            let many = match c.today.len() {
                1 => "one".to_owned(),
                n => n.to_string(),
            };
            let mark = Line::from(vec![
                raw("   "),
                Span::styled("◆", theme::fg(theme::GOOD)),
                dim(format!(
                    " {many} today, {}",
                    format::clock(*first, g.now, g.zone)
                )),
            ]);
            if row.width() + mark.width() <= w {
                row = join([row, mark]);
            }
        }
        if row.width() > w {
            return Err(super::super::text::Overflow(format!(
                "a set's row over {w}"
            )));
        }
        rows.push(row);
    }
    Ok(rows)
}

/// "3 short of a badge: ≈ £0.15 to buy them, at list prices", as fits.
fn short_line(set: &TheSet, w: usize) -> Fits<Line<'static>> {
    if set.missing == 0 {
        return Ok(plain("a full set: a badge can be crafted"));
    }
    let Some(cost) = set.missing_cost else {
        return first_fit([format!("{} short of a badge", set.missing)], w);
    };
    let n = set.missing;
    let cost = format::about(cost);
    first_fit(
        [
            format!("{n} short of a badge: {cost} to buy them, at list prices"),
            format!("{n} short of a badge: {cost} to buy them"),
            format!("{n} short of a badge: {cost}"),
        ],
        w,
    )
}

/// Each card of the set as a cell: its name, never shortened, how many are
/// held, and its price, each column as wide as its widest.
fn cells(set: &TheSet) -> Fits<Vec<Line<'static>>> {
    let name_w = widest(set.cards.iter().map(|c| plain(c.name.clone())));
    let price_w = (widest(set.cards.iter().map(|c| set_price_words(c.normal))) + 2).max(7);
    set.cards
        .iter()
        .map(|c| {
            Ok(join([
                fit(plain(c.name.clone()), name_w)?,
                rfit(count_words(c.owned), 3)?,
                rfit(set_price_words(c.normal), price_w)?,
            ]))
        })
        .collect()
}

/// The set two cards a row, its foils' range after the last.
fn two_a_row(set: &TheSet, w: usize) -> Fits<Vec<Line<'static>>> {
    let cells = cells(set)?;
    let foils = foil_range(set);
    let n = cells.len();
    let half = n.div_ceil(2);
    let mut rows = Vec::new();
    for i in 0..half {
        let right = cells
            .get(half + i)
            .cloned()
            .unwrap_or_else(|| foils.clone());
        rows.push(join([cells[i].clone(), plain("  "), right]));
    }
    if n.is_multiple_of(2) {
        rows.push(foils);
    }
    if rows.iter().any(|r| r.width() > w) {
        return Err(super::super::text::Overflow("two cards a row".into()));
    }
    Ok(rows)
}

/// The set one card a row, then its foils' range.
fn one_a_row(set: &TheSet, w: usize) -> Fits<Vec<Line<'static>>> {
    let mut rows = cells(set)?;
    rows.push(foil_range(set));
    if rows.iter().any(|r| r.width() > w) {
        return Err(super::super::text::Overflow("one card a row".into()));
    }
    Ok(rows)
}

/// "foils £0.35 – £0.60".
fn foil_range(set: &TheSet) -> Line<'static> {
    let values: Vec<market::Money> = set.cards.iter().filter_map(|c| c.foil.value()).collect();
    match (
        values.iter().min_by_key(|m| m.minor),
        values.iter().max_by_key(|m| m.minor),
    ) {
        (Some(lo), Some(hi)) => plain(format!(
            "foils {} – {}",
            format::money(*lo),
            format::money(*hi)
        )),
        _ => plain("foils not priced yet"),
    }
}

/// How the right-hand column at M, `rows` tall, is shared: the chosen
/// game's panel height, and whether this session's cards get the rest. The
/// set's prices beside the queue come first; the haul keeps 7 cards beside
/// a set table, 3 beside a smaller form, or goes (h still opens it).
pub(crate) fn split_m(forms: &[Vec<Line<'static>>], rows: usize) -> (usize, usize, bool) {
    let is_table = |f: &Vec<Line<'static>>| {
        f.iter().any(|l| {
            super::super::text::plain(l)
                .trim_start()
                .starts_with("CARD")
        })
    };
    let priced = if forms.len() > 1 {
        &forms[..forms.len() - 1]
    } else {
        forms
    };
    let fits = priced
        .iter()
        .position(|f| rows.saturating_sub(f.len() + 2) >= if is_table(f) { 9 } else { 5 });
    if let Some(i) = fits {
        return (i, priced[i].len() + 2, true);
    }
    if let Some(i) = priced.iter().position(|f| f.len() + 2 <= rows) {
        return (i, rows, false);
    }
    let last = forms.len().saturating_sub(1);
    let h = forms.last().map_or(rows, |f| f.len() + 2);
    if rows.saturating_sub(h) >= 5 {
        (last, h, true)
    } else {
        (last, rows, false)
    }
}

/// How the right-hand column at M is shared once nothing is left to farm
/// (mockup e): this session's cards are what's left to look at, so the
/// chosen game takes its richest form that leaves them 6, its set folding
/// to one line if need be; when none does, as `split_m` shares it.
pub(crate) fn split_summary(forms: &[Vec<Line<'static>>], rows: usize) -> (usize, usize, bool) {
    match forms.iter().position(|f| f.len() + 2 + 8 <= rows) {
        Some(i) => (i, forms[i].len() + 2, true),
        None => split_m(forms, rows),
    }
}

// ── This session's cards ─────────────────────────────────────────────────────

/// The haul's rows at M, the newest `n`, `w` wide: time, card · game, and
/// price; and how many are earlier. A time on another day has its day, so
/// the times take as many columns as the widest shown.
pub(crate) fn haul_rows_m(
    h: &Haul,
    n: usize,
    spinner: &str,
    w: usize,
) -> Fits<(Vec<Line<'static>>, usize)> {
    let start = h.rows.len().saturating_sub(n);
    let shown = &h.rows[start..];
    let (times, tw) = times(h, shown);
    let mut out = Vec::new();
    for (r, time) in shown.iter().zip(times) {
        let price = price_words(r.price.or(Some(Cell::Pending)));
        let room = w.saturating_sub(tw + 1 + 1 + price.width());
        let what = match &r.card {
            Told::Identifying => first_fit(
                [
                    Line::from(vec![
                        Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                        dim(" which card?"),
                        raw(format!(" · {}", r.game)),
                    ]),
                    Line::from(vec![
                        Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                        dim(" which card?"),
                    ]),
                ],
                room,
            )?,
            card => {
                let c = card_words(card, r.copy, spinner, room.saturating_sub(3 + 8));
                let cw = c.width();
                first_fit(
                    [
                        join([c.clone(), plain(format!(" · {}", r.game))]),
                        join([
                            c.clone(),
                            plain(format!(
                                " · {}",
                                shorten(&r.game, room.saturating_sub(cw + 3).max(4))
                                    .unwrap_or_default()
                            )),
                        ]),
                        c,
                    ],
                    room,
                )?
            }
        };
        let rest = w.saturating_sub(tw + 1 + what.width());
        out.push(join([
            fit(Line::styled(time, theme::dim()), tw)?,
            plain(" "),
            what,
            rfit(price, rest)?,
        ]));
    }
    Ok((out, start))
}

/// The times `rows` dropped at, on the local clock, with the day when it
/// isn't today; and the columns the widest takes, 5 at least.
fn times(h: &Haul, rows: &[HaulRow]) -> (Vec<String>, usize) {
    let times: Vec<String> = rows
        .iter()
        .map(|r| format::clock(r.at, h.now, h.zone))
        .collect();
    let widest = times
        .iter()
        .map(|t| super::super::text::width(t))
        .max()
        .unwrap_or(0)
        .max(5);
    (times, widest)
}

/// The running total after a row: "£0.27", "≥ £1.30".
fn total_words(r: &HaulRow) -> String {
    r.total.map_or_else(String::new, |t| {
        if r.at_least {
            format!("≥ {}", format::money(t))
        } else {
            format::money(t)
        }
    })
}

/// The haul at L, `rows` tall and `w` wide: the track on top when there's
/// room for every card (with its game names when there's more), a table
/// of time, game, card, price and running total, the newest at the bottom,
/// and the other bases below when there's a row for them. Returns the
/// lines, and how many cards are earlier.
pub(crate) fn haul_rows_l(
    h: &Haul,
    rows: usize,
    spinner: &str,
    w: usize,
) -> Fits<(Vec<Line<'static>>, usize)> {
    const GAME: usize = 16;
    const PRICE: usize = 9;
    const TOTAL: usize = 9;
    let n = h.rows.len();
    let top = if rows >= n + 1 + 3 {
        progress::track(&h.track, w, 2, h.zone).unwrap_or_default()
    } else if rows > n + 1 {
        progress::track(&h.track, w, 0, h.zone).unwrap_or_default()
    } else {
        Vec::new()
    };
    let room = rows.saturating_sub(top.len() + 1);
    let start = n.saturating_sub(room);
    let shown = &h.rows[start..];
    let (times, tw) = times(h, shown);
    let cw = w.saturating_sub(tw + 2 + GAME + 2 + 2 + PRICE + 2 + TOTAL);
    let head = Line::styled(
        format!(
            "{:<tw$}  {:<GAME$}  {:<cw$}  {:>PRICE$}  {:>TOTAL$}",
            "TIME",
            "GAME",
            "CARD",
            h_list(h),
            "TOTAL"
        ),
        theme::dim(),
    );
    let body: Vec<Line<'static>> = shown
        .iter()
        .zip(times)
        .map(|(r, time)| {
            Ok(join([
                fit(plain(time), tw)?,
                plain("  "),
                fit(plain(shorten(&r.game, GAME)?), GAME)?,
                plain("  "),
                fit(card_words(&r.card, r.copy, spinner, cw), cw)?,
                plain("  "),
                rfit(price_words(r.price.or(Some(Cell::Pending))), PRICE)?,
                plain("  "),
                rfit(plain(total_words(r)), TOTAL)?,
            ]))
        })
        .collect::<Fits<_>>()?;
    let mut lines = top;
    lines.push(head);
    lines.extend(body);
    if let Some(other) = other_bases(h, w) {
        if rows.saturating_sub(lines.len()) >= 2 {
            lines.push(Line::default());
            lines.push(other);
        } else if rows.saturating_sub(lines.len()) == 1 {
            lines.push(other);
        }
    }
    Ok((lines, start))
}

/// The column's name for prices: "LIST", "NET", "INSTANT".
fn h_list(h: &Haul) -> &'static str {
    match h.basis {
        market::Basis::List => "LIST",
        market::Basis::Net => "NET",
        market::Basis::Instant => "INSTANT",
    }
}

/// "Other bases: ≥ £1.14 after fees · ≥ £0.74 if sold now".
fn other_bases(h: &Haul, w: usize) -> Option<Line<'static>> {
    let at_each = h.at_each?;
    let words = |b: market::Basis| match b {
        market::Basis::List => ("at list prices", "list"),
        market::Basis::Net => ("after fees", "after fees"),
        market::Basis::Instant => ("if sold now", "sold now"),
    };
    let others: Vec<_> = at_each.iter().filter(|(b, _)| *b != h.basis).collect();
    let long = others
        .iter()
        .map(|(b, held)| format!("{} {}", format::at_least(held), words(*b).0))
        .collect::<Vec<_>>()
        .join(" · ");
    let short = others
        .iter()
        .map(|(b, held)| format!("{} {}", format::at_least(held), words(*b).1))
        .collect::<Vec<_>>()
        .join(" · ");
    first_fit([format!("Other bases: {long}"), short], w).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tui::golden::{line_text, mockup},
        viewmodel::{ChosenGame, Haul, Now, Progress, fixtures},
    };

    const SPIN: &str = "⠋";

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

    fn texts(lines: &[Line<'_>]) -> Vec<String> {
        lines
            .iter()
            .map(|l| line_text(l).trim_end().to_owned())
            .collect()
    }

    #[test]
    fn the_now_panel_at_l() {
        let data = fixtures::farming_alone();
        let s = data.snapshot();
        let (rows, since, every) =
            now_panel(&Progress::build(&s, None), &Now::build(&s), SPIN, 72).unwrap();
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (L)", 2..8, 126, 72)
        );
        assert_eq!(since.as_deref(), Some("since 16:53"));
        assert_eq!(
            every.as_deref(),
            Some("looks every 5 min: it's the last card")
        );
    }

    #[test]
    fn the_chosen_game_at_l_and_m() {
        let data = fixtures::farming_alone();
        let g = ChosenGame::build(&data.snapshot(), fixtures::HEAVY_RAIN).unwrap();
        let forms = chosen_forms(&g, 72).unwrap();
        let first = forms.iter().find(|f| f.len() + 2 <= 16).unwrap();
        assert_eq!(
            texts(first),
            inside("dashboard, farming alone (L)", 10..24, 126, 72)
        );

        let forms = chosen_forms(&g, 42).unwrap();
        let (i, h, haul) = split_m(&forms, 21);
        assert_eq!((h, haul), (12, true));
        assert_eq!(
            texts(&forms[i]),
            inside("dashboard, farming alone (M)", 8..18, 76, 42)
        );

        let tall = chosen_forms(&g, 55).unwrap();
        let (i, h, _) = split_m(&tall, 30);
        assert_eq!(h, 16);
        assert_eq!(
            texts(&tall[i]),
            inside("dashboard, farming alone (M, tall)", 9..23, 89, 55)
        );
    }

    #[test]
    fn a_set_not_read_yet() {
        let data = fixtures::building_hours();
        let g = ChosenGame::build(&data.snapshot(), fixtures::STRAY).unwrap();
        let forms = chosen_forms(&g, 42).unwrap();
        assert_eq!(
            texts(&forms[0]),
            inside("building hours with the 12-game group", 8..17, 76, 42)
        );
    }

    #[test]
    fn once_nothing_is_left_the_haul_keeps_six_cards() {
        let data = fixtures::nothing_to_farm();
        let g = ChosenGame::build(&data.snapshot(), fixtures::VAMPIRE_SURVIVORS).unwrap();
        let forms = chosen_forms(&g, 42).unwrap();
        let (i, h, haul) = split_summary(&forms, 18);
        assert_eq!(
            (forms[i].len(), h, haul),
            (7, 9, true),
            "the set in one line"
        );
        assert_eq!(
            texts(&forms[i]),
            inside(
                "nothing left to farm, with the session's summary",
                11..18,
                76,
                42
            )
        );
        assert_eq!(
            split_summary(&forms, 12),
            split_m(&forms, 12),
            "no form leaves 6 cards: as ever"
        );
    }

    #[test]
    fn a_sets_cells_are_as_wide_as_what_they_hold() {
        let data = fixtures::nothing_to_farm();
        let mut g = ChosenGame::build(&data.snapshot(), fixtures::VAMPIRE_SURVIVORS).unwrap();
        let forms = chosen_forms(&g, 72).unwrap();
        assert_eq!(
            texts(&forms[0][3..5]),
            [
                "   CARD           OWNED   NORMAL       FOIL",
                "   Antonio           ×2    £0.05  no market   ◆ 2 today, 15:49"
            ],
            "nobody sells its foils: the column makes room"
        );
        let set = g.set.as_mut().unwrap();
        set.cards[0].name = "Antonio, the Whip Master".into();
        let forms = chosen_forms(&g, 72).unwrap();
        let two = forms
            .iter()
            .find(|f| {
                texts(f)
                    .iter()
                    .any(|l| l.starts_with("Antonio, the Whip Master ×2"))
            })
            .expect("a card's name whole");
        assert!(
            texts(two).iter().all(|l| !l.contains('…')),
            "a card's name is never shortened"
        );
    }

    #[test]
    fn the_haul_at_l_and_m() {
        let data = fixtures::farming_alone();
        let h = Haul::build(&data.snapshot());
        let (rows, start) = haul_rows_l(&h, 21, SPIN, 72).unwrap();
        assert_eq!(start, 0);
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (L)", 26..47, 126, 72)
        );
        let (rows, start) = haul_rows_m(&h, 7, SPIN, 42).unwrap();
        assert_eq!(start, 9, "9 earlier ↑");
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (M)", 20..27, 76, 42)
        );
    }

    #[test]
    fn stale_prices_show_their_age_in_the_haul() {
        let data = fixtures::prices_paused();
        let h = Haul::build(&data.snapshot());
        let (rows, _) = haul_rows_m(&h, 7, SPIN, 42).unwrap();
        assert_eq!(
            texts(&rows),
            inside("prices paused by Steam, some stale", 20..27, 76, 42)
        );
    }

    #[test]
    fn cards_say_which_copy_they_are() {
        let named = |copy| {
            line_text(&card_words(
                &Told::Named {
                    name: "Zagreus".into(),
                    foil: false,
                },
                copy,
                SPIN,
                30,
            ))
        };
        assert_eq!(named(Some(2)), "Zagreus, 2nd copy");
        assert_eq!(named(None), "Zagreus, copy ?");
        assert_eq!(named(Some(1)), "Zagreus");
        let short = line_text(&card_words(
            &Told::Named {
                name: "Madison".into(),
                foil: false,
            },
            Some(2),
            SPIN,
            14,
        ));
        assert_eq!(short, "Madison (2nd)");
        let foil = line_text(&card_words(
            &Told::Named {
                name: "Thanatos".into(),
                foil: true,
            },
            Some(1),
            SPIN,
            22,
        ));
        assert_eq!(foil, "★ Thanatos (foil)");
        assert_eq!(
            line_text(&card_words(&Told::Unknown, None, SPIN, 30)),
            "couldn't tell which card"
        );
        assert_eq!(ordinal(3), "3rd");
        assert_eq!(ordinal(12), "12th");
        assert_eq!(line_text(&price_words(Some(Cell::Failed))), "?");
    }
}
