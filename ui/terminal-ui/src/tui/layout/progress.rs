// Progress's ladders (docs/design/ui.md §2.1, §2.4): at L the big time to
// finish, the session's and library's gauges and the value column; at M four
// rows, five from 36 rows with the session's track; at S three, the library
// in the border; at XS three bare rows. Each gives up what the spec's ladder
// says first. And the glance the too-small screen keeps.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::{
    super::{
        format,
        text::{Fits, big, first_fit, fit, gauge, glue, join, key, rfit, spread, wrap},
        theme,
    },
    SizeClass,
};
use crate::viewmodel::{Activity, Now, Progress, Summary, Track};
use market::Basis;

fn raw(s: impl Into<String>) -> Span<'static> {
    Span::raw(s.into())
}

fn dim(s: impl Into<String>) -> Span<'static> {
    Span::styled(s.into(), theme::dim())
}

fn line(spans: Vec<Span<'static>>) -> Line<'static> {
    Line::from(spans)
}

/// What a state other than farming says: its sentence, what happens next,
/// and its one-line forms, longest first.
struct Message {
    sentence: Line<'static>,
    next: Option<String>,
    lines: Vec<Line<'static>>,
}

fn message(p: &Progress, now: &Now, spinner: &str) -> Option<Message> {
    let at = |t| format::clock(t, p.now, p.zone);
    let busy = theme::fg(theme::BUSY);
    let bad = theme::fg(theme::BAD);
    let said = |text: String, style: Style| line(vec![Span::styled(text, style)]);
    Some(match &p.activity {
        Activity::Farming { .. } | Activity::BuildingHours { .. } => return None,
        Activity::Waiting { by, since } => {
            let by = by.clone().unwrap_or_else(|| "a game".to_owned());
            let since = since.map_or_else(String::new, |t| format!(", since {}", at(t)));
            let next = now.next.as_ref();
            Message {
                sentence: said(
                    format!("‖ Waiting: {by} is being played on another device{since}."),
                    busy,
                ),
                next: Some(match next {
                    Some(g) => format!("Farming carries on a minute after it stops. {g} is next."),
                    None => "Farming carries on a minute after it stops.".to_owned(),
                }),
                lines: vec![
                    said(
                        match next {
                            Some(g) => {
                                format!("‖ Waiting: {by} is played on another device · {g} next")
                            }
                            None => format!("‖ Waiting: {by} is played on another device"),
                        },
                        busy,
                    ),
                    said(format!("‖ Waiting: {by} is played elsewhere"), busy),
                ],
            }
        }
        Activity::Paused => Message {
            sentence: line(vec![
                Span::styled("‖ Paused: nothing is played until you press ", busy),
                key("p"),
                Span::styled(" to carry on.", busy),
            ]),
            next: Some("The time to finish counts farming time, so it waits too.".to_owned()),
            lines: vec![
                line(vec![
                    Span::styled("‖ Paused: nothing is played until you press ", busy),
                    key("p"),
                ]),
                line(vec![
                    Span::styled("‖ Paused: press ", busy),
                    key("p"),
                    Span::styled(" to carry on", busy),
                ]),
            ],
        },
        Activity::Expired { stopped_at } => Message {
            sentence: line(vec![
                Span::styled("✕ Steam no longer takes the saved sign-in: press ", bad),
                key("a"),
                Span::styled(" to sign in again.", bad),
            ]),
            next: Some(match stopped_at {
                Some(t) => format!(
                    "Farming stopped at {}. The queue and this session's cards are kept.",
                    at(*t)
                ),
                None => "Farming stopped. The queue and this session's cards are kept.".to_owned(),
            }),
            lines: vec![
                line(vec![
                    Span::styled("✕ Sign-in expired: press ", bad),
                    key("a"),
                    Span::styled(" to sign in again", bad),
                ]),
                line(vec![
                    Span::styled("✕ Sign-in expired: press ", bad),
                    key("a"),
                ]),
            ],
        },
        Activity::Reconnecting { why, retry_at } => {
            let when = retry_at.map(|t| format::countdown(t, p.now));
            let again = when
                .as_ref()
                .map_or_else(|| "soon".to_owned(), Clone::clone);
            Message {
                sentence: said(
                    format!("✕ Lost touch with Steam: {why}. Trying again {again}."),
                    bad,
                ),
                next: Some(
                    "Nothing is played until it's back; then farming carries on by itself."
                        .to_owned(),
                ),
                lines: vec![
                    said(
                        format!("✕ Lost touch with Steam: trying again {again}"),
                        bad,
                    ),
                    said(format!("✕ Lost touch with Steam: {again}"), bad),
                ],
            }
        }
        Activity::Reading | Activity::Checking => Message {
            sentence: line(vec![
                Span::styled(spinner.to_owned(), busy),
                raw(" Reading your badges for games with cards left"),
            ]),
            next: Some(
                "The first read can take a minute. The time to finish and what the cards are \
                 worth follow as soon as it's done."
                    .to_owned(),
            ),
            lines: vec![
                line(vec![
                    Span::styled(spinner.to_owned(), busy),
                    raw(" Reading your badges for games with cards left"),
                ]),
                line(vec![
                    Span::styled(spinner.to_owned(), busy),
                    raw(" Reading your badges"),
                ]),
            ],
        },
        Activity::Unreadable { retry_at } => {
            let again = retry_at.map_or_else(|| "soon".to_owned(), |t| format::countdown(t, p.now));
            Message {
                sentence: said(
                    format!("✕ Couldn't read your badges: trying again {again}."),
                    bad,
                ),
                next: Some("The queue keeps the last read meanwhile.".to_owned()),
                lines: vec![
                    said(
                        format!("✕ Couldn't read your badges: trying again {again}"),
                        bad,
                    ),
                    said(format!("✕ Badges unread: {again}"), bad),
                ],
            }
        }
        Activity::TakenOver => Message {
            sentence: said(
                "✕ Another session signed in with this account, so farming stopped rather \
                 than knock it off."
                    .to_owned(),
                bad,
            ),
            next: Some("Press p to start again.".to_owned()),
            lines: vec![
                said(
                    "✕ Another session took over: farming stopped".to_owned(),
                    bad,
                ),
                said("✕ Taken over".to_owned(), bad),
            ],
        },
        Activity::NothingToFarm { why, next_look } => {
            // The session's summary says why in its own words.
            let why = p.summary.as_ref().map_or(why, |s| &s.why);
            let when =
                next_look.map_or_else(String::new, |t| format!(" It looks again at {}.", at(t)));
            Message {
                sentence: line(vec![
                    dim("○"),
                    raw(format!(" Nothing left to farm: {why}.{when}")),
                ]),
                next: None,
                lines: vec![line(vec![dim("○"), raw(" Nothing left to farm")])],
            }
        }
        Activity::Stopped => Message {
            sentence: line(vec![
                dim("○"),
                raw(" Not farming: press "),
                key("p"),
                raw(" to start."),
            ]),
            next: None,
            lines: vec![line(vec![dim("○"), raw(" Not farming")])],
        },
        Activity::SignedOut => Message {
            sentence: line(vec![
                dim("○"),
                raw(" Not signed in: press "),
                key("a"),
                raw(" to sign in."),
            ]),
            next: None,
            lines: vec![line(vec![dim("○"), raw(" Not signed in")])],
        },
    })
}

/// What a state other than farming says: its sentence, and what happens
/// next. `None` while a game or a group is farmed.
pub(crate) fn sentence(
    p: &Progress,
    now: &Now,
    spinner: &str,
) -> Option<(Line<'static>, Option<String>)> {
    message(p, now, spinner).map(|m| (m.sentence, m.next))
}

/// How values are shown: "list prices", "after fees", "sold now".
fn basis_words(basis: Basis) -> &'static str {
    match basis {
        Basis::List => "list prices",
        Basis::Net => "after fees",
        Basis::Instant => "sold now",
    }
}

/// The time to finish in words, longest first: "≈ 4d 21h to finish ·
/// around Sun 4 Oct · 80%: 3d 13h – 6d 15h", or "of farming left" while it
/// holds still, or the assumption before the second drop.
pub(crate) fn eta_words(p: &Progress) -> Vec<Line<'static>> {
    let Some(e) = &p.eta else {
        return Vec::new();
    };
    let eta = format::eta(e.eta);
    let bold = |s: String| Span::styled(s, theme::bold());
    let hours = (!e.hours_term.is_zero()).then(|| {
        format!(
            "incl. ≈ {} of building hours",
            format::estimate(e.hours_term)
        )
    });
    match e.band.filter(|_| !e.assumed) {
        None => {
            let mut out = Vec::new();
            if let Some(h) = &hours {
                out.push(line(vec![
                    bold(eta.clone()),
                    raw(format!(" to finish, assuming 30 min a drop · {h}")),
                ]));
            }
            out.push(line(vec![
                bold(eta.clone()),
                raw(" to finish, assuming 30 min a drop"),
            ]));
            out.push(line(vec![bold(eta), raw(" to finish (30 min a drop)")]));
            out
        }
        Some(band) => {
            let (what, date) = if e.holds_still {
                (" of farming left", String::new())
            } else {
                (
                    " to finish",
                    format!(" · around {}", format::date(e.lands, p.zone)),
                )
            };
            let band = format::band(band);
            vec![
                line(vec![
                    bold(eta.clone()),
                    raw(format!("{what}{date} · {band}")),
                ]),
                line(vec![bold(eta.clone()), raw(format!("{what} · {band}"))]),
                line(vec![bold(eta.clone()), raw(format!("{what} ({band})"))]),
                line(vec![bold(eta), raw(what)]),
            ]
        }
    }
}

/// Progress's title, in its top border, longest first: what the rate was
/// learnt from, a first estimate, "starting", or the session's span.
pub(crate) fn progress_title(p: &Progress) -> Vec<Line<'static>> {
    if let Some(summary) = &p.summary {
        return vec![line(vec![dim(format!(
            "{} – {}",
            format::day_and_time(summary.from, p.zone),
            format::day_and_time(summary.to, p.zone)
        ))])];
    }
    if matches!(p.activity, Activity::Reading) {
        return vec![line(vec![dim("starting")])];
    }
    let Some(e) = &p.eta else {
        return vec![line(vec![dim("starting")])];
    };
    if e.band.is_none() || e.assumed {
        return vec![
            line(vec![dim(
                "a first estimate: a likely range follows the second card",
            )]),
            line(vec![dim("a first estimate")]),
        ];
    }
    let rate = format::rate(e.rate);
    let from = format!("{} in {}", e.learnt.drops, format::duration(e.learnt.alone));
    let hours = (!e.hours_term.is_zero()).then(|| {
        format!(
            " · incl. ≈ {} of building hours",
            format::estimate(e.hours_term)
        )
    });
    let mut out = Vec::new();
    if let Some(h) = &hours {
        out.push(format!("{rate}, learnt from {from} farming alone{h}"));
        out.push(format!("{rate}, from {from} alone{h}"));
    }
    out.push(format!("{rate}, learnt from {from} farming alone"));
    out.push(format!("{rate}, from {from}"));
    out.push(rate);
    out.into_iter().map(|t| line(vec![dim(t)])).collect()
}

/// Progress's title at S: the state's word, the library's drops and games,
/// or since when, as fit.
pub(crate) fn progress_title_s(p: &Progress) -> Vec<Line<'static>> {
    let lib = format!(
        "library {} of {} drops",
        p.library.received, p.library.total
    );
    let games = format!(
        "{lib} · {} of {} games",
        p.library.games_done, p.library.games
    );
    let word = match &p.activity {
        Activity::Waiting { .. } => Some("waiting"),
        Activity::Paused => Some("paused"),
        Activity::Expired { .. } => Some("stopped"),
        Activity::Reconnecting { .. } => Some("reconnecting"),
        Activity::Reading => return vec![line(vec![dim("starting")])],
        _ if p.eta.as_ref().is_some_and(|e| e.assumed) => {
            return vec![line(vec![dim("a first estimate")])];
        }
        _ => None,
    };
    let texts = match word {
        Some(w) => vec![
            format!("{w} · {games}"),
            format!("{w} · {lib}"),
            w.to_owned(),
        ],
        None => {
            let since = p.session.started_at.map_or_else(String::new, |t| {
                format!("since {}", format::clock(t, p.now, p.zone))
            });
            vec![games, lib, since]
        }
    };
    texts.into_iter().map(|t| line(vec![dim(t)])).collect()
}

/// A count, a thin gauge and a tail: "This session  16 of 252 drops
/// ━━──  6% · 5 of 62 games". The counts are padded to 17 so stacked gauges
/// start in the same column; under 8 columns of gauge, the gauge goes.
fn gauged(
    label: &str,
    head: String,
    done: u32,
    of: u32,
    tail: String,
    w: usize,
    style: Style,
) -> Fits<Line<'static>> {
    const LABEL: usize = 14;
    let head_w = super::super::text::width(&head).max(17);
    let g = w as isize - (LABEL + head_w) as isize - super::super::text::width(&tail) as isize;
    let label = fit(Line::from(label.to_owned()), LABEL)?;
    if g < 8 {
        return Ok(join([
            label,
            Line::from(format!("{} ·{}", head.trim_end(), tail.replace("  ", " "))),
        ]));
    }
    Ok(join([
        label,
        fit(Line::from(head), head_w)?,
        gauge(done, of, g as usize, style),
        Line::from(tail),
    ]))
}

/// The session's and the library's gauges, each on its own row.
fn gauges(p: &Progress, w: usize) -> Fits<[Line<'static>; 2]> {
    let s = &p.session;
    let of = s.drops_at_start.unwrap_or(0);
    let pct = |k, n| format!("{:>3}", format::percent(k, n));
    let session = gauged(
        "This session",
        format!("{} of {} drops ", s.drops, of),
        s.drops,
        of,
        format!(
            " {} · {} of {} games",
            pct(s.drops, of),
            s.games_done,
            s.games_at_start.unwrap_or(0)
        ),
        w,
        theme::fg(theme::GOOD),
    )?;
    let l = &p.library;
    let library = gauged(
        "Your library",
        format!("{} of {} drops ", l.received, l.total),
        l.received,
        l.total,
        format!(
            " {} · {} of {} games",
            pct(l.received, l.total),
            l.games_done,
            l.games
        ),
        w,
        Style::new(),
    )?;
    Ok([session, library])
}

/// The value column's heading, with the key that changes the basis: "VALUE ·
/// list prices  [b]".
fn value_heading(p: &Progress) -> Line<'static> {
    line(vec![
        Span::styled(
            format!(
                "VALUE · {}",
                p.values
                    .as_ref()
                    .map_or("list prices", |v| basis_words(v.basis))
            ),
            theme::heading(),
        ),
        raw("  "),
        key("b"),
    ])
}

/// The value column, five rows: its heading, this session's value, what's
/// still to drop and the value on completion (or how pricing goes), and
/// the spares. While Steam has paused lookups, the pause takes a row.
fn value_rows(p: &Progress, spinner: &str, at_l: bool) -> Vec<Line<'static>> {
    let heading = value_heading(p);
    let none_yet = || line(vec![dim("no cards yet this session")]);
    if matches!(p.activity, Activity::Reading) {
        return vec![
            heading,
            none_yet(),
            line(vec![dim("prices follow the badges")]),
            Line::default(),
            Line::default(),
        ];
    }
    let Some(v) = &p.values else {
        return vec![
            heading,
            none_yet(),
            line(vec![dim("prices follow the wallet's currency")]),
            Line::default(),
            Line::default(),
        ];
    };
    let held = if p.session.drops == 0 {
        none_yet()
    } else {
        let mut spans = vec![
            Span::styled(format::at_least(&v.held), theme::bold()),
            raw(" this session"),
        ];
        if v.held.unpriced > 0 {
            spans.push(raw(" · "));
            spans.push(Span::styled(
                format!("{} unpriced", v.held.unpriced),
                theme::fg(theme::BUSY),
            ));
        }
        line(spans)
    };
    let after_fees = if v.left.basis == v.basis {
        ""
    } else {
        ", after fees"
    };
    let oldest = |d: Option<std::time::Duration>| {
        d.map_or_else(String::new, |d| format!(" · oldest {}", format::age(d)))
    };
    let left = line(vec![raw(format!(
        "{} still to drop{after_fees}",
        format::about(v.left.value)
    ))]);
    let completion = line(vec![
        raw(format::about(v.completion.value)),
        dim(" on completion, excl. foils"),
    ]);
    let spares = if v.spares.0 > 0 {
        line(vec![raw(format!(
            "{} this session: {}",
            format::spares(v.spares.0),
            format::money(v.spares.1.total)
        ))])
    } else {
        Line::default()
    };
    if !v.all_priced() {
        return vec![
            heading,
            held,
            line(vec![
                Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                raw(format!(
                    " pricing your games: {} of {}",
                    v.pricing.priced, v.pricing.of
                )),
            ]),
            line(vec![dim("on completion: once they're priced")]),
            spares,
        ];
    }
    match p.pause {
        Some(pause) if at_l => vec![
            heading,
            held,
            line(vec![raw(format!(
                "{} still to drop{}",
                format::about(v.left.value),
                oldest(v.left_oldest)
            ))]),
            completion,
            line(vec![Span::styled(
                format!(
                    "‖ prices paused until {}",
                    format::clock(pause.until, p.now, p.zone)
                ),
                theme::fg(theme::BUSY),
            )]),
        ],
        Some(pause) => vec![
            heading,
            held,
            line(vec![raw(format!(
                "{} on completion{}",
                format::about(v.completion.value),
                oldest(v.held.oldest.max(v.left_oldest))
            ))]),
            line(vec![Span::styled(
                format!(
                    "‖ prices paused by Steam until {}",
                    format::clock(pause.until, p.now, p.zone)
                ),
                theme::fg(theme::BUSY),
            )]),
            spares,
        ],
        None => vec![heading, held, left, completion, spares],
    }
}

/// The Now line, below L: the game farmed and its pips, the next look and
/// its last card; building hours, and when the lead has its 3 hours; or
/// what the state says, in a line.
pub(crate) fn now_line(p: &Progress, now: &Now, spinner: &str, w: usize) -> Fits<Line<'static>> {
    if let Some(m) = message(p, now, spinner) {
        return first_fit(m.lines, w);
    }
    let good = theme::fg(theme::GOOD);
    if let Some(group) = &now.group {
        let n = group.games.len();
        let lead = &group.games[0];
        let ready = format::estimate_at(
            p.now,
            (group.lead_ready - p.now).to_std().unwrap_or_default(),
            p.zone,
        );
        let what = if n == 1 {
            lead.name.clone()
        } else {
            format!("{n} games")
        };
        return first_fit(
            [
                line(vec![
                    Span::styled("▷", good),
                    raw(format!(
                        " Building hours on {what} together · {} has 3h ≈ {ready}, then plays alone",
                        lead.name
                    )),
                ]),
                line(vec![
                    Span::styled("▷", good),
                    raw(format!(
                        " Building hours on {what} · {} has 3h ≈ {ready}",
                        lead.name
                    )),
                ]),
                line(vec![
                    Span::styled("▷", good),
                    raw(format!(" Building hours on {what}")),
                ]),
            ],
            w,
        );
    }
    let Some(g) = &now.game else {
        return Ok(Line::default());
    };
    let pips = super::super::text::pips(g.pips.before, g.pips.today, g.pips.foils, g.pips.total);
    let look = now.next_look.map_or_else(String::new, |t| {
        format!(" · next look {}", format::countdown(t, p.now))
    });
    let card = match (g.first_card, g.last_card) {
        (Some(t), _) => format!(" · first card ≈ {}", at_clock(p, t)),
        (None, Some(t)) => format!(" · last card ≈ {}", at_clock(p, t)),
        _ => String::new(),
    };
    let counts = format::of(g.received, g.total);
    let name = |rest: String, with_pips: bool| {
        let mut spans = vec![Span::styled("▶ ", good), raw(g.name.clone()), raw(" ")];
        if with_pips {
            spans.extend(pips.spans.clone());
            spans.push(raw(" "));
        }
        spans.push(raw(rest));
        line(spans)
    };
    first_fit(
        [
            name(format!("{counts}{look}{card}"), true),
            name(format!("{counts}{look}{card}"), false),
            name(format!("{counts}{look}"), true),
            line(vec![
                Span::styled("▶ ", good),
                raw(format!("{}{look}", g.name)),
            ]),
        ],
        w,
    )
}

/// An estimate's time on the local clock, rounded to its step.
fn at_clock(p: &Progress, t: chrono::DateTime<chrono::Utc>) -> String {
    format::estimate_at(p.now, (t - p.now).to_std().unwrap_or_default(), p.zone)
}

/// The summary row while farming waits: the time left, the drops and a
/// gauge, the value.
pub(crate) fn summary_line(p: &Progress, w: usize) -> Fits<Line<'static>> {
    let eta = p
        .eta
        .as_ref()
        .map_or_else(String::new, |e| format::eta(e.eta));
    let s = &p.session;
    let of = s.drops_at_start.unwrap_or(0);
    let value = p
        .values
        .as_ref()
        .map_or_else(String::new, |v| format!(" · {}", format::held(&v.held)));
    let head = format!("{eta} of farming left · {} of {of} drops ", s.drops);
    let tail = format!(" {}{value}", format::percent(s.drops, of));
    let g = w as isize
        - super::super::text::width(&head) as isize
        - super::super::text::width(&tail) as isize;
    if g >= 6 {
        return Ok(join([
            Line::from(head),
            gauge(s.drops, of, g as usize, theme::fg(theme::GOOD)),
            Line::from(tail),
        ]));
    }
    first_fit(
        [
            format!("{} · {}", head.trim_end(), tail.trim()),
            format!("{eta} left · {} of {of} drops{value}", s.drops),
            format!("{eta} left{value}"),
        ],
        w,
    )
}

/// Progress at L: five rows of the big time to finish, the gauges, and the
/// value column, 34, then whatever's left, then 36 columns.
pub(crate) fn progress_l(
    p: &Progress,
    now: &Now,
    spinner: &str,
    w: usize,
) -> Fits<Vec<Line<'static>>> {
    const E: usize = 34;
    const V: usize = 36;
    if let Some(m) = message(p, now, spinner).filter(|_| matches!(p.activity, Activity::Reading)) {
        let mut rows = vec![m.sentence];
        for l in wrap(m.next.unwrap_or_default(), w - 2, 0)? {
            rows.push(join([Line::from("  "), l]));
        }
        rows.resize(5, Line::default());
        return Ok(rows);
    }
    let Some(e) = &p.eta else {
        return Ok(vec![Line::default(); 5]);
    };
    let pw = w.saturating_sub(E + V + 4);
    let g = pw.saturating_sub(5);
    let digits = big(&format::estimate(e.eta));
    let bold = |s: String| Span::styled(s, theme::bold());
    let eta_rows: Vec<Line<'static>> = if e.assumed || e.band.is_none() {
        vec![
            line(vec![raw("  "), bold(digits[0].clone())]),
            line(vec![raw("≈ "), bold(digits[1].clone()), raw("  to finish")]),
            line(vec![raw("  "), bold(digits[2].clone())]),
            line(vec![dim("assuming 30 min a drop, for now:")]),
            line(vec![dim("a range follows the second card")]),
        ]
    } else if e.holds_still {
        vec![
            line(vec![raw("  "), bold(digits[0].clone())]),
            line(vec![
                raw("≈ "),
                bold(digits[1].clone()),
                raw("  of farming"),
            ]),
            line(vec![raw("  "), bold(digits[2].clone()), raw("  left")]),
            line(vec![dim("counted in farming time")]),
            line(vec![raw(format::band(e.band.unwrap_or_default()))]),
        ]
    } else {
        vec![
            line(vec![raw("  "), bold(digits[0].clone())]),
            line(vec![raw("≈ "), bold(digits[1].clone()), raw("  to finish")]),
            line(vec![raw("  "), bold(digits[2].clone())]),
            line(vec![raw(format!(
                "around {}, if left running",
                format::date(e.lands, p.zone)
            ))]),
            line(vec![raw(format::band(e.band.unwrap_or_default()))]),
        ]
    };
    let s = &p.session;
    let l = &p.library;
    let of = s.drops_at_start.unwrap_or(0);
    let label = |t: &str| fit(Line::from(t.to_owned()), 14);
    let to_go = |aside: bool| -> Fits<Line<'static>> {
        let mut text = format!(
            "{} · {}",
            format::games(p.to_go.games),
            format::drops(p.to_go.drops)
        );
        if aside && p.to_go.set_aside > 0 {
            text.push_str(&format!(" · {} set aside", p.to_go.set_aside));
        }
        Ok(join([label("To go")?, Line::from(text)]))
    };
    let prog = [
        join([
            label("This session")?,
            Line::from(format!(
                "{} of {of} drops · {} of {} games",
                s.drops,
                s.games_done,
                s.games_at_start.unwrap_or(0)
            )),
        ]),
        join([
            gauge(s.drops, of, g, theme::fg(theme::GOOD)),
            rfit(format::percent(s.drops, of), 5)?,
        ]),
        join([
            label("Your library")?,
            Line::from(format!(
                "{} of {} drops · {} of {} games",
                l.received, l.total, l.games_done, l.games
            )),
        ]),
        join([
            gauge(l.received, l.total, g, Style::new()),
            rfit(format::percent(l.received, l.total), 5)?,
        ]),
        first_fit([to_go(true)?, to_go(false)?], pw)?,
    ];
    let val = value_rows(p, spinner, true);
    (0..5)
        .map(|i| {
            Ok(join([
                fit(eta_rows[i].clone(), E)?,
                Line::from("  "),
                fit(prog[i].clone(), pw)?,
                Line::from("  "),
                fit(val[i].clone(), V)?,
            ]))
        })
        .collect()
}

/// Progress at M: four rows, five with the session's track: the time to
/// finish, the session, the library, the Now line, beside the value
/// column's 37.
pub(crate) fn progress_m(
    p: &Progress,
    now: &Now,
    track: Option<&Track>,
    spinner: &str,
    w: usize,
    rows: usize,
) -> Fits<Vec<Line<'static>>> {
    const V: usize = 37;
    let lw = w.saturating_sub(V + 2);
    let left: Vec<Line<'static>> = if matches!(p.activity, Activity::Reading) {
        let m = message(p, now, spinner).expect("a message while reading");
        let mut out = vec![m.sentence];
        for l in wrap(m.next.unwrap_or_default(), lw - 2, 0)? {
            out.push(join([Line::from("  "), l]));
        }
        out.resize(rows.max(out.len()), Line::default());
        out.truncate(rows);
        out
    } else {
        let [session, library] = gauges(p, lw)?;
        let mut out = vec![
            first_fit(eta_words(p), lw)?,
            session,
            library,
            now_line(p, now, spinner, lw)?,
        ];
        if rows == 5 {
            out.push(
                match track.filter(|_| p.eta.as_ref().is_some_and(|e| !e.assumed)) {
                    Some(t) => track_line(t, lw, p.zone)?,
                    None => Line::from("A likely range appears once two cards have dropped."),
                },
            );
        }
        out
    };
    let val = value_rows(p, spinner, false);
    (0..rows)
        .map(|i| {
            Ok(join([
                fit(left[i].clone(), lw)?,
                Line::from("  "),
                fit(val.get(i).cloned().unwrap_or_default(), V)?,
            ]))
        })
        .collect()
}

/// Progress at S: three rows. While farming waits, what it waits for and a
/// summary; otherwise the time to finish and the value, the session's
/// drops and its gauge, and the Now line.
pub(crate) fn progress_s(
    p: &Progress,
    now: &Now,
    spinner: &str,
    w: usize,
) -> Fits<Vec<Line<'static>>> {
    if let Some(m) = message(p, now, spinner) {
        if matches!(p.activity, Activity::Reading) {
            let mut rows = vec![m.sentence];
            for l in wrap(m.next.unwrap_or_default(), w - 2, 0)? {
                rows.push(join([Line::from("  "), l]));
            }
            rows.resize(3, Line::default());
            rows.truncate(3);
            return Ok(rows);
        }
        let mut first = vec![m.sentence];
        first.extend(m.lines);
        let next = m
            .next
            .map_or(Line::default(), |n| Line::from(format!("  {n}")));
        return Ok(vec![
            first_fit(first, w)?,
            first_fit([next, Line::default()], w)?,
            summary_line(p, w)?,
        ]);
    }
    let Some(e) = &p.eta else {
        return Ok(vec![Line::default(); 3]);
    };
    let s = &p.session;
    let of = s.drops_at_start.unwrap_or(0);
    let Some(v) = &p.values else {
        return Ok(vec![
            first_fit(eta_words(p), w)?,
            Line::default(),
            now_line(p, now, spinner, w)?,
        ]);
    };
    if e.assumed || !v.all_priced() {
        let pricing = format!(
            "⠋ pricing your games: {} of {}",
            v.pricing.priced, v.pricing.of
        );
        let hours = format!("{} to finish, assuming 30 min a drop", format::eta(e.eta));
        return Ok(vec![
            first_fit(
                [
                    format!(
                        "{hours}, incl. ≈ {} of hours",
                        format::estimate(e.hours_term)
                    ),
                    hours,
                ],
                w,
            )?,
            first_fit(
                [
                    format!("{} of {of} drops · no cards yet · {pricing}", s.drops),
                    format!(
                        "{} of {of} drops · ⠋ pricing: {} of {}",
                        s.drops, v.pricing.priced, v.pricing.of
                    ),
                ],
                w,
            )?,
            now_line(p, now, spinner, w)?,
        ]);
    }
    let eta = format::eta(e.eta);
    let band = e.band.map(format::band).unwrap_or_default();
    let top = spread(
        first_fit(
            [
                format!("{eta} to finish · {band}"),
                format!("{eta} to finish"),
            ],
            w.saturating_sub(24),
        )?,
        format::held(&v.held),
        w,
        1,
    )?;
    let head = format!("{} of {of} drops ", s.drops);
    let pct = format::percent(s.drops, of);
    let done = format::about(v.completion.value);
    let tail = first_fit(
        [
            format!(
                " {pct} · {} of {} games · {done} on completion",
                s.games_done,
                s.games_at_start.unwrap_or(0)
            ),
            format!(" {pct} · {done} on completion"),
            format!(" {pct} · {done} when done"),
        ],
        w.saturating_sub(super::super::text::width(&head) + 6),
    )?;
    let g = w - super::super::text::width(&head) - tail.width();
    Ok(vec![
        top,
        join([
            Line::from(head),
            gauge(s.drops, of, g, theme::fg(theme::GOOD)),
            tail,
        ]),
        now_line(p, now, spinner, w)?,
    ])
}

/// Progress at XS: three bare rows, the time to finish and the drops, the
/// value, and the Now line.
pub(crate) fn progress_xs(
    p: &Progress,
    now: &Now,
    spinner: &str,
    w: usize,
) -> Fits<Vec<Line<'static>>> {
    if matches!(p.activity, Activity::Reading) {
        return Ok(vec![
            line(vec![
                Span::styled(spinner.to_owned(), theme::fg(theme::BUSY)),
                raw(" Reading your badges for games with cards left"),
            ]),
            Line::from("The first read can take a minute."),
            Line::default(),
        ]);
    }
    let Some(e) = &p.eta else {
        return Ok(vec![Line::default(); 3]);
    };
    let s = &p.session;
    let of = s.drops_at_start.unwrap_or(0);
    let eta = format::eta(e.eta);
    let what = if e.holds_still {
        "of farming left"
    } else {
        "to finish"
    };
    let Some(v) = &p.values else {
        return Ok(vec![
            Line::from(format!("{eta} {what}")),
            Line::default(),
            now_line(p, now, spinner, w)?,
        ]);
    };
    if e.assumed || !v.all_priced() {
        return Ok(vec![
            first_fit(
                [
                    format!("{eta} to finish, assuming 30 min a drop"),
                    format!("{eta} to finish (30 min a drop)"),
                ],
                w,
            )?,
            first_fit(
                [
                    format!(
                        "{} of {of} drops · ⠋ pricing your games: {} of {}",
                        s.drops, v.pricing.priced, v.pricing.of
                    ),
                    format!("{} of {of} drops · ⠋ pricing", s.drops),
                ],
                w,
            )?,
            now_line(p, now, spinner, w)?,
        ]);
    }
    let done = format::about(v.completion.value);
    Ok(vec![
        first_fit(
            [
                format!(
                    "{eta} {what} · {} of {of} drops · {} of {} games",
                    s.drops,
                    s.games_done,
                    s.games_at_start.unwrap_or(0)
                ),
                format!("{eta} {what} · {} of {of} drops", s.drops),
            ],
            w,
        )?,
        first_fit(
            [
                format!("{} · {done} on completion", format::held(&v.held)),
                format!("{} · {done} when done", format::held(&v.held)),
            ],
            w,
        )?,
        now_line(p, now, spinner, w)?,
    ])
}

/// Progress's rows at a size class, `w` wide and `rows` tall: the session
/// summed up once nothing is left to farm, why nothing is when nothing
/// dropped, else the class's own ladder. `track` is the session's, at M and
/// L.
pub(crate) fn panel_rows(
    p: &Progress,
    now: &Now,
    track: Option<&Track>,
    spinner: &str,
    class: SizeClass,
    w: usize,
    rows: usize,
) -> Fits<Vec<Line<'static>>> {
    if p.summary.is_some() {
        return summary_rows(p, track, w, rows);
    }
    if matches!(p.activity, Activity::NothingToFarm { .. }) {
        return idle_rows(p, now, spinner, w, rows);
    }
    match class {
        SizeClass::L => progress_l(p, now, spinner, w),
        SizeClass::M => progress_m(p, now, track, spinner, w, rows),
        SizeClass::S => progress_s(p, now, spinner, w),
        SizeClass::Xs | SizeClass::TooSmall => progress_xs(p, now, spinner, w),
    }
}

/// Progress's title, for its top border, in `w` columns: the session's span
/// once it's summed up; at S the state's word and the library, which the
/// panel has no row for; above S what the rate was learnt from.
pub(crate) fn panel_title(p: &Progress, class: SizeClass, w: usize) -> Option<Line<'static>> {
    let options = if class == SizeClass::S && p.summary.is_none() {
        progress_title_s(p)
    } else {
        progress_title(p)
    };
    first_fit(options, w).ok()
}

/// The summary's three columns' widths at M and L: what the session did and
/// how its first estimate did; its value takes the rest.
const SESSION_COLUMN: usize = 38;
const ESTIMATE_COLUMN: usize = 40;

/// Progress once nothing is left to farm (mockup e): the session summed up.
/// At M, seven rows: why nothing is left and when the farmer looks again;
/// three columns, what the session did, how its first estimate did against
/// what happened, and what its cards are worth; and the session's track. At
/// L, five: the columns and the track, the Now panel saying why. At S and
/// XS, three: why, the drops, the value.
pub(crate) fn summary_rows(
    p: &Progress,
    track: Option<&Track>,
    w: usize,
    rows: usize,
) -> Fits<Vec<Line<'static>>> {
    let Some(sm) = &p.summary else {
        return Ok(vec![Line::default(); rows]);
    };
    let of = p.session.drops_at_start.unwrap_or(sm.drops);
    let took = format::duration((sm.to - sm.from).to_std().unwrap_or_default());
    let rate = sm
        .rate
        .map_or_else(String::new, |r| format!(" · {}", format::rate(r)));
    if rows <= 3 {
        let value = sm.value.as_ref().map_or_else(String::new, format::held);
        let foils = match sm.foils.len() {
            0 => String::new(),
            1 => " · ★ 1 foil".to_owned(),
            n => format!(" · ★ {n} foils"),
        };
        let estimated = sm.estimated.map_or_else(String::new, |m| {
            format!(" · estimated {}", format::about(m))
        });
        let mut out = vec![
            first_fit(summary_sentence(p, sm), w)?,
            first_fit(
                [
                    format!(
                        "{} of {of} drops from {} in {took}{rate}",
                        sm.drops,
                        format::games(sm.games)
                    ),
                    format!("{} of {of} drops in {took}", sm.drops),
                    format!("{} of {of} drops", sm.drops),
                ],
                w,
            )?,
            first_fit(
                [
                    format!("{value}{foils}{estimated}"),
                    format!("{value}{foils}"),
                    value,
                ],
                w,
            )?,
        ];
        out.truncate(rows);
        return Ok(out);
    }
    let session = [
        Line::styled("THIS SESSION", theme::heading()),
        Line::from(format!(
            "{} of {of} drops, from {}",
            sm.drops,
            format::games(sm.games)
        )),
        Line::from(format!("in {took}{rate}")),
        Line::from(format!(
            "Your library: {} of {} drops",
            p.library.received, p.library.total
        )),
    ];
    // The columns at their own widths when there's room; narrower, each as
    // wide as its words, the estimate's taking what's left; narrower still,
    // without the estimate.
    let needs = value_width(p, sm);
    let (session_w, estimate) = if w >= SESSION_COLUMN + ESTIMATE_COLUMN + needs + 4 {
        (
            SESSION_COLUMN,
            Some((ESTIMATE_COLUMN, estimate_column(p, sm, ESTIMATE_COLUMN)?)),
        )
    } else {
        let session_w = session.iter().map(Line::width).max().unwrap_or(0);
        let room = w.saturating_sub(session_w + needs + 4);
        let estimate = (room >= 24)
            .then(|| estimate_column(p, sm, room).ok())
            .flatten()
            .map(|lines| (room, lines));
        (session_w, estimate)
    };
    let value_w = match &estimate {
        Some((ew, _)) => w.saturating_sub(session_w + ew + 4),
        None => w.saturating_sub(session_w + 2),
    };
    let value = value_column(p, sm, value_w)?;
    let mut columns = Vec::new();
    for (i, said) in session.iter().enumerate() {
        let mut row = vec![fit(said.clone(), session_w)?, Line::from("  ")];
        if let Some((ew, lines)) = &estimate {
            row.push(fit(lines[i].clone(), *ew)?);
            row.push(Line::from("  "));
        }
        row.push(fit(value[i].clone(), value_w)?);
        columns.push(join(row));
    }
    let days = match track {
        Some(t) => self::track(t, w, 1, p.zone)?,
        None => Vec::new(),
    };
    let mut out = Vec::new();
    if rows >= 7 {
        out.push(first_fit(summary_sentence(p, sm), w)?);
        out.extend(columns);
        out.extend(days);
    } else {
        out.extend(columns);
        out.extend(days.into_iter().take(1));
    }
    out.resize(rows, Line::default());
    Ok(out)
}

/// Why nothing is left to farm, and when the farmer looks again, longest
/// first: "○ Nothing left to farm: all done or skipped. It looks again at
/// Mon 01:44, or as soon as you rank or unskip a game."
fn summary_sentence(p: &Progress, sm: &Summary) -> Vec<Line<'static>> {
    let lead = format!(" Nothing left to farm: {}.", sm.why);
    let mut out = Vec::new();
    if let Some(at) = sm.next_look {
        let at = format::clock(at, p.now, p.zone);
        out.push(format!(
            "{lead} It looks again at {at}, or as soon as you rank or unskip a game."
        ));
        out.push(format!("{lead} It looks again at {at}."));
    }
    out.push(lead);
    out.push(" Nothing left to farm".to_owned());
    out.into_iter()
        .map(|t| line(vec![dim("○"), raw(t)]))
        .collect()
}

/// The summary's estimate column, four rows `w` wide: its heading, then what
/// the first estimate with a likely range said, made after the second card,
/// against what it took from then, wrapped.
fn estimate_column(p: &Progress, sm: &Summary, w: usize) -> Fits<Vec<Line<'static>>> {
    let options: Vec<String> = match sm.estimate {
        Some(c) => {
            let made = c.made_at.with_timezone(&p.zone);
            let said = format!(
                "said {} at {} on {}, after the second card",
                glue(&format::eta(c.said)),
                made.format("%H:%M"),
                made.format("%a")
            );
            let took = format!("it took {} from then", glue(&format::duration(c.took)));
            let mut options = Vec::new();
            if let Some((low, high)) = c.band {
                let within = if c.inside { "inside" } else { "outside" };
                options.push(format!(
                    "{said}; {took}, {within} its 80% band, {}",
                    glue(&format!(
                        "{} – {}",
                        format::duration(low),
                        format::duration(high)
                    ))
                ));
            }
            options.push(format!("{said}; {took}"));
            options.push(format!("said {}; {took}", glue(&format::eta(c.said))));
            options
        }
        None => vec!["no first estimate: it takes two cards farmed alone".to_owned()],
    };
    let body = options
        .into_iter()
        .filter_map(|o| wrap(o, w, 0).ok())
        .find(|lines| lines.len() <= 3)
        .ok_or_else(|| {
            super::super::text::Overflow(format!("the estimate's words over 3 rows of {w}"))
        })?;
    let mut out = vec![Line::styled("THE ESTIMATE", theme::heading())];
    out.extend(body);
    out.resize(4, Line::default());
    Ok(out)
}

/// How wide the summary's value column needs to be: as wide as its widest
/// row, its foils named or only counted.
fn value_width(p: &Progress, sm: &Summary) -> usize {
    value_column(p, sm, usize::MAX)
        .map(|rows| {
            let foils_counted = match sm.foils.len() {
                0 => 0,
                1 => super::super::text::width("★ 1 foil"),
                n => super::super::text::width(&format!("★ {n} foils")),
            };
            rows.iter()
                .enumerate()
                .map(|(i, r)| if i == 2 { foils_counted } else { r.width() })
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

/// The summary's value column, four rows `w` wide: its heading, this
/// session's cards and how many aren't priced, its foils, and what the
/// first estimate said they'd be worth.
fn value_column(p: &Progress, sm: &Summary, w: usize) -> Fits<Vec<Line<'static>>> {
    let held = match &sm.value {
        Some(h) if h.unpriced > 0 => line(vec![
            Span::styled(format::at_least(h), theme::bold()),
            raw(" · "),
            Span::styled(
                match h.unpriced {
                    1 => "1 card not priced".to_owned(),
                    n => format!("{n} cards not priced"),
                },
                theme::fg(theme::BUSY),
            ),
        ]),
        Some(h) => line(vec![Span::styled(format::at_least(h), theme::bold())]),
        None => Line::default(),
    };
    let star = || Span::styled("★", theme::fg(theme::ACCENT));
    let foils = match sm.foils.len() {
        0 => Line::default(),
        n => {
            let count = if n == 1 {
                "1 foil".to_owned()
            } else {
                format!("{n} foils")
            };
            first_fit(
                [
                    line(vec![
                        star(),
                        raw(format!(" {count}: {}", sm.foils.join(", "))),
                    ]),
                    line(vec![star(), raw(format!(" {count}"))]),
                ],
                w,
            )?
        }
    };
    let estimated = sm.estimated.map_or_else(Line::default, |m| {
        line(vec![
            raw(format!("estimated {}", format::about(m))),
            dim(", excl. foils"),
        ])
    });
    Ok(vec![value_heading(p), held, foils, estimated])
}

/// Progress when nothing is left to farm and nothing dropped this session:
/// why, and when the farmer looks again, then the library's drops and games.
fn idle_rows(
    p: &Progress,
    now: &Now,
    spinner: &str,
    w: usize,
    rows: usize,
) -> Fits<Vec<Line<'static>>> {
    let mut why = Vec::new();
    if let Some(m) = message(p, now, spinner) {
        why.push(m.sentence);
        why.extend(m.lines);
    }
    let l = &p.library;
    let mut out = vec![
        first_fit(why, w)?,
        first_fit(
            [
                format!(
                    "Your library: {} of {} drops · {} of {} games",
                    l.received, l.total, l.games_done, l.games
                ),
                format!("Your library: {} of {} drops", l.received, l.total),
                format!("library {} of {}", l.received, l.total),
            ],
            w,
        )?,
    ];
    out.resize(rows, Line::default());
    Ok(out)
}

/// The glance the too-small screen keeps: what's farming and its next look,
/// the drops and the time to finish, and the value so far.
pub(crate) fn glance(p: &Progress, now: &Now, spinner: &str, w: usize) -> Vec<Line<'static>> {
    let state = match (&now.game, message(p, now, spinner)) {
        (Some(g), None) => {
            let look = now.next_look.map_or_else(String::new, |t| {
                format!(" · next look {}", format::countdown(t, p.now))
            });
            first_fit(
                [
                    format!("▶ {} · {}{look}", g.name, format::of(g.received, g.total)),
                    format!("▶ {}{look}", g.name),
                ],
                w,
            )
            .ok()
        }
        (None, None) => now_line(p, now, spinner, w).ok(),
        (_, Some(m)) => first_fit(m.lines, w).ok(),
    };
    let drops = p.eta.as_ref().map(|e| {
        let of = p.session.drops_at_start.unwrap_or(0);
        let what = if e.holds_still {
            "of farming left"
        } else {
            "to finish"
        };
        format!(
            "{} of {of} drops · {} {what}",
            p.session.drops,
            format::eta(e.eta)
        )
    });
    let value = p.values.as_ref().filter(|_| p.session.drops > 0).map(|v| {
        if v.held.unpriced > 0 {
            format!(
                "{} so far · {} unpriced",
                format::at_least(&v.held),
                v.held.unpriced
            )
        } else {
            format!("{} so far", format::at_least(&v.held))
        }
    });
    [state, drops.map(Line::from), value.map(Line::from)]
        .into_iter()
        .flatten()
        .filter(|l| l.width() <= w)
        .collect()
}

/// The session as one line `w` wide, with its start and "now" at either
/// end: ◆ a drop, ★ a foil, ┼ one game handing over to the next.
pub(crate) fn track_line(t: &Track, w: usize, zone: chrono::FixedOffset) -> Fits<Line<'static>> {
    Ok(track(t, w, 0, zone)?.remove(0))
}

/// The session's track, `w` wide, with `labels` rows of game names under
/// it (0 to 2), each name centred under its game's stretch and left out
/// if it doesn't fit whole. A session longer than a day draws its days and
/// foils instead.
pub(crate) fn track(
    t: &Track,
    w: usize,
    labels: usize,
    zone: chrono::FixedOffset,
) -> Fits<Vec<Line<'static>>> {
    if t.by_day {
        return track_by_day(t, w, zone);
    }
    let left = format!("{} ", format::clock(t.from, t.to, zone));
    let right = format!(" {} now", format::clock(t.to, t.to, zone));
    let lw = super::super::text::width(&left);
    let aw = w as isize - lw as isize - super::super::text::width(&right) as isize;
    if aw < 20 {
        return Err(super::super::text::Overflow(format!("a track {aw} wide")));
    }
    let aw = aw as usize;
    let span = (t.to - t.from).num_milliseconds().max(1) as f64;
    let pos = |at: chrono::DateTime<chrono::Utc>| -> usize {
        let x = (at - t.from).num_milliseconds() as f64 / span * (aw - 1) as f64 + 0.5;
        (x.floor().max(0.0) as usize).min(aw - 1)
    };
    let mut axis = vec!['─'; aw];
    for (_, from, _) in t.games.iter().skip(1) {
        axis[pos(*from)] = '┼';
    }
    for &(at, foil) in &t.marks {
        let mut c = pos(at);
        while c < aw - 1 && matches!(axis[c], '◆' | '★') {
            c += 1;
        }
        if axis[c] == '┼' {
            let next = if c + 1 < aw && axis[c + 1] == '─' {
                c + 1
            } else {
                c - 1
            };
            axis[next] = '┼';
        }
        axis[c] = if foil { '★' } else { '◆' };
    }
    let mut spans = vec![dim(left)];
    for ch in axis {
        spans.push(match ch {
            '◆' => Span::styled("◆", theme::fg(theme::GOOD)),
            '★' => Span::styled("★", theme::fg(theme::ACCENT)),
            c => dim(c.to_string()),
        });
    }
    spans.push(dim(right));
    let mut out = vec![line(spans)];
    let mut rows = vec![vec![' '; w]; labels];
    let mut ends = vec![-2_isize; labels];
    for (i, (name, from, to)) in t.games.iter().enumerate() {
        if labels == 0 {
            break;
        }
        let r = i % labels;
        let mid = lw + (pos(*from) + pos(*to)) / 2;
        let n = super::super::text::width(name);
        let start = mid.saturating_sub(n / 2).max(lw).min(w.saturating_sub(n));
        if start as isize <= ends[r] + 1 {
            continue;
        }
        for (k, ch) in name.chars().enumerate() {
            rows[r][start + k] = ch;
        }
        ends[r] = (start + n) as isize - 1;
    }
    out.extend(rows.into_iter().map(|r| {
        line(vec![dim(r
            .into_iter()
            .collect::<String>()
            .trim_end()
            .to_owned())])
    }));
    Ok(out)
}

/// A session longer than a day: its days, ┼ at each midnight, ★ each foil,
/// and each day's name under it.
fn track_by_day(t: &Track, w: usize, zone: chrono::FixedOffset) -> Fits<Vec<Line<'static>>> {
    let left = format!("{} ", format::day_and_time(t.from, zone));
    let right = format!(" {}", format::day_and_time(t.to, zone));
    let lw = super::super::text::width(&left);
    let aw = w as isize - lw as isize - super::super::text::width(&right) as isize;
    if aw < 20 {
        return Err(super::super::text::Overflow(format!("a track {aw} wide")));
    }
    let aw = aw as usize;
    let span = (t.to - t.from).num_milliseconds().max(1) as f64;
    let pos = |at: chrono::DateTime<chrono::Utc>| -> usize {
        let x = (at - t.from).num_milliseconds() as f64 / span * (aw - 1) as f64 + 0.5;
        (x.floor().max(0.0) as usize).min(aw - 1)
    };
    let local = t.from.with_timezone(&zone);
    let first_midnight = local
        .date_naive()
        .succ_opt()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .and_then(|d| d.and_local_timezone(zone).single())
        .map(|d| d.to_utc());
    let mut midnights = Vec::new();
    let mut m = first_midnight;
    while let Some(at) = m.filter(|at| *at < t.to) {
        midnights.push(at);
        m = Some(at + chrono::TimeDelta::days(1));
    }
    let mut axis = vec!['─'; aw];
    for at in &midnights {
        axis[pos(*at)] = '┼';
    }
    for &(at, foil) in &t.marks {
        if foil {
            axis[pos(at)] = '★';
        }
    }
    let mut spans = vec![dim(left)];
    for ch in axis {
        spans.push(match ch {
            '★' => Span::styled("★", theme::fg(theme::ACCENT)),
            c => dim(c.to_string()),
        });
    }
    spans.push(dim(right));
    let mut labels = vec![' '; w];
    let mut bounds = vec![t.from];
    bounds.extend(midnights);
    bounds.push(t.to);
    for pair in bounds.windows(2) {
        let x = (lw + (pos(pair[0]) + pos(pair[1])) / 2).saturating_sub(1);
        for (k, ch) in format::weekday(pair[0], zone).chars().enumerate() {
            if x + k < w {
                labels[x + k] = ch;
            }
        }
    }
    Ok(vec![
        line(spans),
        line(vec![dim(labels
            .into_iter()
            .collect::<String>()
            .trim_end()
            .to_owned())]),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tui::golden::{line_text, mockup},
        viewmodel::{Haul, fixtures},
    };

    const SPIN: &str = "⠋";

    fn built(data: &fixtures::DataSet) -> (Progress, Now, Track) {
        let s = data.snapshot();
        (
            Progress::build(&s, None),
            Now::build(&s),
            Haul::build(&s).track,
        )
    }

    /// The rows inside a mockup's panel: `from` to `to`, less the border and
    /// its padding column each side.
    fn inside(title: &str, rows: std::ops::Range<usize>, x: usize, w: usize) -> Vec<String> {
        let m = mockup(title);
        m.rows[rows]
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
    fn progress_at_l_is_the_spec_s() {
        let data = fixtures::farming_alone();
        let (p, now, _) = built(&data);
        let rows = progress_l(&p, &now, SPIN, 120).unwrap();
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (L)", 2..7, 2, 120)
        );
        let title = first_fit(progress_title(&p), 124 - 16).unwrap();
        assert_eq!(
            line_text(&title),
            "2.1 drops an hour, learnt from 16 in 7h 40m farming alone · incl. ≈ 3h of building hours"
        );
    }

    #[test]
    fn progress_at_l_in_the_users_window_says_what_is_set_aside() {
        let data = fixtures::farming_alone();
        let (p, now, _) = built(&data);
        let rows = progress_l(&p, &now, SPIN, 129).unwrap();
        assert_eq!(
            texts(&rows),
            inside(
                "dashboard, farming alone, the user's window (L)",
                2..7,
                2,
                129
            )
        );
    }

    #[test]
    fn progress_at_m_is_the_spec_s() {
        let data = fixtures::farming_alone();
        let (p, now, track) = built(&data);
        let rows = progress_m(&p, &now, Some(&track), SPIN, 116, 4).unwrap();
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (M)", 2..6, 2, 116)
        );
        let tall = progress_m(&p, &now, Some(&track), SPIN, 142, 5).unwrap();
        assert_eq!(
            texts(&tall),
            inside("dashboard, farming alone (M, tall)", 2..7, 2, 142)
        );
    }

    #[test]
    fn progress_at_s_and_xs_is_the_spec_s() {
        let data = fixtures::farming_alone();
        let (p, now, _) = built(&data);
        let rows = progress_s(&p, &now, SPIN, 76).unwrap();
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (S)", 2..5, 2, 76)
        );
        let title = first_fit(progress_title_s(&p), 80 - 16).unwrap();
        assert_eq!(
            line_text(&title),
            "library 183 of 421 drops · 5 of 63 games"
        );
        let xs = progress_xs(&p, &now, SPIN, 58).unwrap();
        assert_eq!(
            texts(&xs),
            inside("dashboard, farming alone (XS)", 1..4, 1, 58)
        );
    }

    #[test]
    fn progress_while_farming_waits() {
        for (data, title) in [
            (
                fixtures::waiting_for_hades(),
                "Steam played on another device",
            ),
            (fixtures::paused(), "paused"),
            (fixtures::sign_in_expired(), "sign-in expired"),
            (fixtures::reconnecting(), "connection lost, retrying"),
        ] {
            let (p, now, _) = built(&data);
            let rows = progress_s(&p, &now, SPIN, 76).unwrap();
            assert_eq!(texts(&rows), inside(title, 2..5, 2, 76), "{title}");
        }
    }

    #[test]
    fn progress_while_the_badges_are_read() {
        let data = fixtures::reading_badges();
        let (p, now, _) = built(&data);
        let m = progress_m(&p, &now, None, SPIN, 116, 4).unwrap();
        assert_eq!(texts(&m), inside("reading the badges (M)", 2..6, 2, 116));
        let s = progress_s(&p, &now, SPIN, 76).unwrap();
        assert_eq!(texts(&s), inside("reading the badges (S)", 2..5, 2, 76));
        let xs = progress_xs(&p, &now, SPIN, 58).unwrap();
        assert_eq!(texts(&xs), inside("reading the badges (XS)", 1..4, 1, 58));
    }

    #[test]
    fn progress_in_the_first_minutes_and_with_prices_paused() {
        let data = fixtures::first_minutes();
        let (p, now, track) = built(&data);
        let rows = progress_m(&p, &now, Some(&track), SPIN, 116, 4).unwrap();
        assert_eq!(
            texts(&rows),
            inside("the first minutes of a session", 2..6, 2, 116)
        );
        let title = first_fit(progress_title(&p), 104).unwrap();
        assert_eq!(
            line_text(&title),
            "a first estimate: a likely range follows the second card"
        );

        let data = fixtures::prices_paused();
        let (p, now, track) = built(&data);
        let rows = progress_m(&p, &now, Some(&track), SPIN, 116, 4).unwrap();
        assert_eq!(
            texts(&rows),
            inside("prices paused by Steam, some stale", 2..6, 2, 116)
        );
    }

    #[test]
    fn building_hours_says_when_the_lead_has_its_hours() {
        let data = fixtures::building_hours();
        let (p, now, track) = built(&data);
        let rows = progress_m(&p, &now, Some(&track), SPIN, 116, 4).unwrap();
        assert_eq!(
            texts(&rows)[3],
            inside("building hours with the 12-game group", 5..6, 2, 116)[0]
        );
    }

    #[test]
    fn the_glance_the_too_small_screen_keeps() {
        let data = fixtures::farming_alone();
        let (p, now, _) = built(&data);
        assert_eq!(
            texts(&glance(&p, &now, SPIN, 50)),
            [
                "▶ Heavy Rain · 3 of 4 · next look in 4m",
                "16 of 252 drops · ≈ 4d 21h to finish",
                "≥ £1.45 so far · 3 unpriced"
            ]
        );
        assert_eq!(
            texts(&glance(&p, &now, SPIN, 30)),
            [
                "▶ Heavy Rain · next look in 4m",
                "≥ £1.45 so far · 3 unpriced"
            ],
            "what fits"
        );
    }

    #[test]
    fn the_session_summed_up_at_every_size() {
        let data = fixtures::nothing_to_farm();
        let s = data.snapshot();
        let estimated = market::Money::new(1_679, market::Currency::GBP);
        let p = Progress::build(&s, Some(estimated));
        let now = Now::build(&s);
        let track = Haul::build(&s).track;
        let at = |class, w, rows| {
            texts(&panel_rows(&p, &now, Some(&track), SPIN, class, w, rows).unwrap())
        };
        assert_eq!(
            at(SizeClass::M, 116, 7),
            inside(
                "nothing left to farm, with the session's summary",
                2..9,
                2,
                116
            )
        );
        let l = at(SizeClass::L, 120, 5);
        assert!(l[0].starts_with("THIS SESSION"), "{l:?}");
        assert!(
            l[4].starts_with("Tue 09:14 ─"),
            "the track, its days unnamed"
        );
        assert_eq!(
            at(SizeClass::S, 76, 3),
            [
                "○ Nothing left to farm: all done or skipped. It looks again at Mon 01:44.",
                "252 of 252 drops from 62 games in 5d 8h · 2.1 drops an hour",
                "≥ £17.31 · 4 unpriced · ★ 2 foils · estimated ≈ £16.79"
            ]
        );
        assert_eq!(
            at(SizeClass::Xs, 58, 3),
            [
                "○ Nothing left to farm: all done or skipped.",
                "252 of 252 drops in 5d 8h",
                "≥ £17.31 · 4 unpriced · ★ 2 foils · estimated ≈ £16.79"
            ]
        );
        // Each column as wide as its words, the estimate's in what's left.
        let narrow = at(SizeClass::M, 96, 7);
        let columns = |a: &str, b: &str, c: &str| format!("{a:<31}  {b:<30}  {c}");
        assert_eq!(
            narrow[1..5],
            [
                columns("THIS SESSION", "THE ESTIMATE", "VALUE · list prices  [b]"),
                columns(
                    "252 of 252 drops, from 62 games",
                    "said ≈ 5d 6h at 10:12 on Tue,",
                    "≥ £17.31 · 4 cards not priced"
                ),
                columns(
                    "in 5d 8h · 2.1 drops an hour",
                    "after the second card; it took",
                    "★ 2 foils: Thanatos, The Lamb"
                ),
                columns(
                    "Your library: 419 of 421 drops",
                    "5d 8h from then",
                    "estimated ≈ £16.79, excl. foils"
                ),
            ]
        );
    }

    #[test]
    fn nothing_left_to_farm_with_no_cards_says_why() {
        let mut data = fixtures::nothing_to_farm();
        data.status.session.drops.clear();
        let s = data.snapshot();
        let p = Progress::build(&s, None);
        assert_eq!(p.summary, None, "nothing to sum up");
        let rows = panel_rows(&p, &Now::build(&s), None, SPIN, SizeClass::M, 116, 4).unwrap();
        assert_eq!(
            texts(&rows),
            [
                "○ Nothing left to farm: every game with cards left is skipped. It looks again at \
                 Mon 01:44.",
                "Your library: 419 of 421 drops · 62 of 63 games",
                "",
                ""
            ]
        );
    }

    #[test]
    fn the_session_track_on_its_day_and_over_a_week() {
        let data = fixtures::farming_alone();
        let (_, _, track) = built(&data);
        let rows = super::track(&track, 72, 2, fixtures::zone()).unwrap();
        assert_eq!(
            texts(&rows),
            inside("dashboard, farming alone (L)", 26..29, 126, 72)
        );
        let week = fixtures::nothing_to_farm();
        let (_, _, track) = built(&week);
        let rows = super::track(&track, 116, 0, fixtures::zone()).unwrap();
        assert_eq!(
            texts(&rows),
            inside(
                "nothing left to farm, with the session's summary",
                7..9,
                2,
                116
            )
        );
    }
}
