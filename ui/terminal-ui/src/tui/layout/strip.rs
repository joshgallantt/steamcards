// The strip and the footer's ladders (docs/design/ui.md §2.1, §2.4, §5.8).
// The strip gives up an event's detail first, then shortens an alert; the
// time, the glyph and what happened stay. The footer's hints go by priority,
// the first that doesn't fit ending the row; help and quit go last.

use ratatui::text::{Line, Span};

use super::{
    super::{
        format,
        text::{Fits, Hint, first_fit, hints},
        theme,
    },
    SizeClass,
};
use crate::viewmodel::{Activity, Alert, Detail, LogEntry, LogKind, Progress, Strip};

/// A log line's glyph's colour: ✓ a drop and ▶ playing GOOD, ↻ moved on
/// LINK, ‖ waiting and ! a warning BUSY, ✕ an error BAD, · the rest DIM.
pub(crate) fn glyph_style(kind: LogKind) -> ratatui::style::Style {
    match kind {
        LogKind::Dropped | LogKind::Playing => theme::fg(theme::GOOD),
        LogKind::MovedOn => theme::fg(theme::LINK),
        LogKind::Waiting | LogKind::Warning => theme::fg(theme::BUSY),
        LogKind::Error => theme::fg(theme::BAD),
        LogKind::Info | LogKind::Progress => theme::dim(),
    }
}

/// " 17:23  ✓ " and what follows.
fn event(
    entry: &LogEntry,
    p: &Progress,
    text: String,
    style: ratatui::style::Style,
) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!(" {}  ", format::clock(entry.at, p.now, p.zone)),
            theme::dim(),
        ),
        Span::styled(format!("{} ", entry.kind.glyph()), glyph_style(entry.kind)),
        Span::styled(text, style),
    ])
}

/// The strip, `w` wide: a flash, an event with what's still being found out
/// about it, or an alert, each as fits.
pub(crate) fn strip(s: &Strip, p: &Progress, w: usize) -> Fits<Line<'static>> {
    match s {
        Strip::Empty => Ok(Line::default()),
        Strip::Flash { text, failed } => first_fit(
            [Line::from(vec![
                Span::styled(" ▸ ", theme::fg(theme::ACCENT)),
                Span::styled(
                    text.clone(),
                    if *failed {
                        theme::strong(theme::BAD)
                    } else {
                        theme::bold()
                    },
                ),
            ])],
            w,
        ),
        Strip::Event { entry, detail } => {
            let plain = ratatui::style::Style::new();
            let mut options = Vec::new();
            match detail {
                Some(Detail::Identifying) => {
                    options.push(event(
                        entry,
                        p,
                        format!("{} · finding out which card", entry.text),
                        plain,
                    ));
                }
                Some(Detail::Unknown) => {
                    options.push(event(
                        entry,
                        p,
                        format!("{} · couldn't tell which card", entry.text),
                        plain,
                    ));
                }
                None => {}
            }
            options.push(event(entry, p, entry.text.clone(), plain));
            first_fit(options, w)
        }
        Strip::Alert(Alert::PricesPaused(entry)) => {
            let until = p.pause.map(|x| format::clock(x.until, p.now, p.zone));
            let mut options = vec![event(
                entry,
                p,
                entry.text.clone(),
                ratatui::style::Style::new(),
            )];
            if let Some(until) = until {
                options.push(event(
                    entry,
                    p,
                    format!("Prices wait until {until}: farming carries on."),
                    ratatui::style::Style::new(),
                ));
                options.push(event(
                    entry,
                    p,
                    format!("Prices wait until {until}"),
                    ratatui::style::Style::new(),
                ));
            }
            first_fit(options, w)
        }
        Strip::Alert(Alert::Reconnecting { at, why }) => {
            let entry = LogEntry {
                at: at.unwrap_or(p.now),
                kind: LogKind::Error,
                text: String::new(),
            };
            first_fit(
                [
                    event(
                        &entry,
                        p,
                        format!("Lost touch with Steam ({why}): trying every minute"),
                        ratatui::style::Style::new(),
                    ),
                    event(
                        &entry,
                        p,
                        "Lost touch with Steam: trying every minute".to_owned(),
                        ratatui::style::Style::new(),
                    ),
                ],
                w,
            )
        }
        Strip::Alert(Alert::Expired { at }) => {
            let entry = LogEntry {
                at: at.unwrap_or(p.now),
                kind: LogKind::Error,
                text: String::new(),
            };
            first_fit(
                [event(
                    &entry,
                    p,
                    "Steam no longer takes the saved sign-in".to_owned(),
                    ratatui::style::Style::new(),
                )],
                w,
            )
        }
    }
}

/// The footer's hints, `w` wide, with a space before them: the dashboard's
/// keys, by priority; while the sign-in has expired, signing in again
/// comes first of all.
pub(crate) fn footer(
    class: SizeClass,
    activity: &Activity,
    done_shown: bool,
    w: usize,
) -> Fits<Line<'static>> {
    let paused = matches!(activity, Activity::Paused);
    let expired = matches!(activity, Activity::Expired { .. });
    let pairs = [
        Hint::new(
            "↑↓",
            if class == SizeClass::L {
                "choose game"
            } else {
                "choose"
            },
            1,
        ),
        Hint::new("enter", "details", 2),
        Hint::new("1-9", "rank", 9),
        Hint::new("x", "skip", 11),
        Hint::new("c", if done_shown { "hide done" } else { "show done" }, 12),
        Hint::divider(3),
        Hint::new("h", "haul", 4),
        Hint::new("m", "market", 8),
        Hint::new("b", "list/net/instant", 10),
        Hint::new("t", "times", 13),
        Hint::new("g", "games", 7),
        if expired {
            Hint::new("a", "sign in again", 0)
        } else {
            Hint::new("a", "account", 6)
        },
        Hint::new("l", "log", 8),
        Hint::new("p", if paused { "carry on" } else { "pause" }, 5),
        Hint::new("?", "help", 0),
        Hint::new("q", "quit", 0),
    ];
    let mut line = hints(&pairs, w.saturating_sub(1))?;
    line.spans.insert(0, Span::raw(" "));
    Ok(line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tui::golden::{line_text, mockup},
        viewmodel::{Flash, Progress, fixtures},
    };

    fn last_two(title: &str) -> (String, String) {
        let m = mockup(title);
        let n = m.rows.len();
        (m.rows[n - 2].clone(), m.rows[n - 1].clone())
    }

    #[test]
    fn the_footer_at_every_size_the_spec_draws() {
        let data = fixtures::farming_alone();
        let activity = Activity::of(&data.snapshot());
        for (title, class) in [
            ("dashboard, farming alone (L)", SizeClass::L),
            (
                "dashboard, farming alone, the user's window (L)",
                SizeClass::L,
            ),
            ("dashboard, farming alone (M, tall)", SizeClass::M),
            ("dashboard, farming alone (M)", SizeClass::M),
            ("dashboard, farming alone (S)", SizeClass::S),
            ("dashboard, farming alone (XS)", SizeClass::Xs),
        ] {
            let w = usize::from(mockup(title).width);
            let line = footer(class, &activity, false, w).unwrap();
            assert_eq!(line_text(&line).trim_end(), last_two(title).1, "{title}");
        }
    }

    #[test]
    fn the_footer_puts_signing_in_again_first() {
        let data = fixtures::sign_in_expired();
        let line = footer(SizeClass::S, &Activity::of(&data.snapshot()), false, 80).unwrap();
        assert_eq!(line_text(&line).trim_end(), last_two("sign-in expired").1);
        let paused = fixtures::paused();
        let line = footer(SizeClass::S, &Activity::of(&paused.snapshot()), false, 80).unwrap();
        assert_eq!(line_text(&line).trim_end(), last_two("paused").1);
    }

    #[test]
    fn the_strip_says_the_newest_event_and_gives_up_its_detail_first() {
        let data = fixtures::farming_alone();
        let s = data.snapshot();
        let p = Progress::build(&s, None);
        let entry = LogEntry {
            at: fixtures::at(17, 23),
            kind: LogKind::Dropped,
            text: "A card dropped for Heavy Rain — 1 to go".into(),
        };
        let shown = Strip::build(&s, None, Some((&entry, false)), None);
        assert_eq!(
            line_text(&strip(&shown, &p, 200).unwrap()),
            last_two("dashboard, farming alone (L)").0
        );
        assert_eq!(
            line_text(&strip(&shown, &p, 60).unwrap()),
            last_two("dashboard, farming alone (XS)").0
        );
        let flash = Strip::build(
            &s,
            Some(Flash {
                text: "Paused: nothing is played until you carry on. Press p.".into(),
                failed: false,
            }),
            None,
            None,
        );
        assert_eq!(
            line_text(&strip(&flash, &p, 80).unwrap()),
            last_two("paused").0
        );
    }

    #[test]
    fn an_alert_shortens_as_the_window_narrows() {
        let data = fixtures::prices_paused();
        let s = data.snapshot();
        let p = Progress::build(&s, None);
        let paused = LogEntry {
            at: fixtures::at(16, 41),
            kind: LogKind::Waiting,
            text:
                "Steam turned down price lookups again: they wait until 17:41. Farming carries on."
                    .into(),
        };
        let shown = Strip::build(&s, None, None, Some(&paused));
        assert_eq!(
            line_text(&strip(&shown, &p, 120).unwrap()),
            last_two("prices paused by Steam, some stale").0
        );
        assert_eq!(
            line_text(&strip(&shown, &p, 60).unwrap()),
            " 16:41  ‖ Prices wait until 17:41: farming carries on."
        );
        let expired = fixtures::sign_in_expired();
        let s = expired.snapshot();
        let shown = Strip::build(&s, None, None, None);
        assert_eq!(
            line_text(&strip(&shown, &Progress::build(&s, None), 80).unwrap()),
            last_two("sign-in expired").0
        );
    }
}
