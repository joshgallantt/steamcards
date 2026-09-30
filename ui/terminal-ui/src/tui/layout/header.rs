// The header's ladder (docs/design/ui.md §2.4): the pill, the farming state
// and for how long, the account, how friends see it, keeping awake, and
// help. As the window narrows it gives up, in order: "keeping awake", "·
// since 09:14", the word "Steam", a long note shortens, the account's name,
// the note shortens again, the word "for", the help label, the note, the
// time, and the help key, which the footer has. The pill, the state and
// "appears offline" never go.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::super::{
    format,
    text::{key, spread},
    theme,
};
use crate::viewmodel::{Activity, Header};

/// What the header gives up, in order.
const GIVES_UP: [Drop; 11] = [
    Drop::Awake,
    Drop::Since,
    Drop::Steam,
    Drop::LongNote,
    Drop::Account,
    Drop::MiddleNote,
    Drop::For,
    Drop::HelpLabel,
    Drop::ShortNote,
    Drop::Time,
    Drop::HelpKey,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drop {
    Awake,
    Since,
    Steam,
    LongNote,
    Account,
    MiddleNote,
    For,
    HelpLabel,
    ShortNote,
    Time,
    HelpKey,
}

/// The farming state's word and its colour, whether it runs (so has a
/// time), and its note, longest first.
fn state(h: &Header) -> ((&'static str, Style), bool, Vec<String>) {
    let good = theme::fg(theme::GOOD);
    let busy = theme::fg(theme::BUSY);
    let bad = theme::fg(theme::BAD);
    let dim = theme::dim();
    let retry = |at: Option<chrono::DateTime<chrono::Utc>>| -> Vec<String> {
        at.map_or_else(Vec::new, |at| {
            let when = format::countdown(at, h.now);
            vec![format!("trying again {when}"), when.clone(), when]
        })
    };
    match &h.activity {
        Activity::Farming { .. }
        | Activity::BuildingHours { .. }
        | Activity::Reading
        | Activity::Checking => (("● farming", good), true, Vec::new()),
        Activity::Paused => (("‖ paused", busy), false, Vec::new()),
        Activity::Waiting { by, .. } => {
            let notes = match by {
                Some(game) => vec![
                    format!("{game} is being played on another device"),
                    format!("{game} is played elsewhere"),
                    format!("{game} elsewhere"),
                ],
                None => vec![
                    "another device is playing".to_owned(),
                    "played elsewhere".to_owned(),
                    "elsewhere".to_owned(),
                ],
            };
            (("‖ waiting", busy), false, notes)
        }
        Activity::NothingToFarm { .. } => (("○ nothing to farm", dim), false, Vec::new()),
        Activity::Expired { .. } | Activity::Stopped => (("○ not farming", dim), false, Vec::new()),
        Activity::SignedOut => (("○ not signed in", dim), false, Vec::new()),
        Activity::Reconnecting { retry_at, .. } => {
            (("✕ reconnecting", bad), false, retry(*retry_at))
        }
        Activity::Unreadable { retry_at } => (("✕ badges unread", bad), false, retry(*retry_at)),
        Activity::TakenOver => (
            ("✕ stopped", bad),
            false,
            vec![
                "another session took over".to_owned(),
                "taken over".to_owned(),
                "taken over".to_owned(),
            ],
        ),
    }
}

/// The farming state's word, in its colour: "● farming", "‖ paused".
pub(crate) fn state_word(h: &Header) -> Span<'static> {
    let ((word, style), _, _) = state(h);
    Span::styled(word, style)
}

/// Each rung of the header's ladder, fullest first: its left and right
/// sides.
fn rungs(h: &Header) -> Vec<(Line<'static>, Line<'static>)> {
    let ((word, word_style), runs, notes) = state(h);
    let dim = theme::dim();
    let mut out: Vec<(Line<'static>, Line<'static>)> = Vec::new();
    let mut said: Vec<(String, String)> = Vec::new();
    for n in 0..=GIVES_UP.len() {
        let gone = &GIVES_UP[..n];
        let has = |d: Drop| !gone.contains(&d);
        let mut left = vec![Span::styled(word, word_style)];
        if let (true, Some(since)) = (runs && has(Drop::Time), h.since) {
            let lasted = (h.now - since).to_std().unwrap_or_default();
            let lead = if has(Drop::For) { " for " } else { " " };
            left.push(Span::styled(
                format!("{lead}{}", format::duration(lasted)),
                dim,
            ));
            if has(Drop::Since) {
                left.push(Span::styled(
                    format!(" · since {}", format::clock(since, h.now, h.zone)),
                    dim,
                ));
            }
        }
        let note = if has(Drop::LongNote) {
            notes.first()
        } else if has(Drop::MiddleNote) {
            notes.get(1)
        } else if has(Drop::ShortNote) {
            notes.get(2)
        } else {
            None
        };
        if let Some(note) = note.filter(|n| !n.is_empty()) {
            left.push(Span::styled(format!(" · {note}"), dim));
        }
        let mut right: Vec<Span<'static>> = Vec::new();
        if h.account.is_some() && (has(Drop::Account) || h.expired) {
            if has(Drop::Steam) {
                right.push(Span::styled("Steam", theme::steam()));
                right.push(Span::raw(" "));
            }
            if h.expired {
                let words = if has(Drop::Account) {
                    "✕ sign-in expired"
                } else {
                    "✕ expired"
                };
                right.push(Span::styled(words, theme::fg(theme::BAD)));
            } else {
                right.push(Span::styled("●", theme::fg(theme::GOOD)));
                right.push(Span::raw(format!(
                    " {}",
                    h.account
                        .as_deref()
                        .filter(|a| !a.is_empty())
                        .unwrap_or("signed in")
                )));
            }
            right.push(Span::styled(" · ", dim));
        }
        right.push(Span::styled(
            if h.appear_online {
                "online"
            } else {
                "appears offline"
            },
            dim,
        ));
        if runs && h.keeping_awake && has(Drop::Awake) {
            right.push(Span::styled(" · keeping awake", dim));
        }
        if has(Drop::HelpKey) {
            right.push(Span::raw(if has(Drop::HelpLabel) { "   " } else { "  " }));
            right.push(key("?"));
            if has(Drop::HelpLabel) {
                right.push(Span::styled(" help", dim));
            }
        }
        let (l, r) = (Line::from(left), Line::from(right));
        let text = (super::super::text::plain(&l), super::super::text::plain(&r));
        if said.last() != Some(&text) {
            said.push(text);
            out.push((l, r));
        }
    }
    out
}

/// The header, exactly `w` wide: the fullest rung that fits, the pill's
/// padding tightening from three spaces to two before a rung is given up.
pub(crate) fn header(w: usize, h: &Header) -> Line<'static> {
    let pill = || Span::styled(" steamcards ", theme::pill());
    for (left, right) in rungs(h) {
        for pad in ["  ", " "] {
            let mut l = Line::from(vec![pill(), Span::raw(pad)]);
            l.spans.extend(left.spans.clone());
            if l.width() + 3 + right.width() <= w
                && let Ok(line) = spread(l, right.clone(), w, 3)
            {
                return line;
            }
        }
    }
    // Narrower than any rung: the pill and the state alone.
    Line::from(vec![pill()])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tui::golden::{line_text, mockup},
        viewmodel::fixtures,
    };

    fn row(title: &str) -> String {
        mockup(title).rows[0].clone()
    }

    fn at(data: &fixtures::DataSet, w: usize) -> String {
        line_text(&header(w, &Header::build(&data.snapshot())))
    }

    #[test]
    fn the_header_at_every_size_the_spec_draws() {
        let data = fixtures::farming_alone();
        for title in [
            "dashboard, farming alone (L)",
            "dashboard, farming alone, the user's window (L)",
            "dashboard, farming alone (M, tall)",
            "dashboard, farming alone (M)",
            "dashboard, farming alone (S)",
            "dashboard, farming alone (XS)",
        ] {
            let m = mockup(title);
            assert_eq!(
                at(&data, usize::from(m.width)).trim_end(),
                row(title),
                "{title}"
            );
        }
    }

    #[test]
    fn the_header_says_what_farming_is_doing() {
        let cases = [
            (
                fixtures::waiting_for_hades(),
                "Steam played on another device",
            ),
            (fixtures::paused(), "paused"),
            (fixtures::sign_in_expired(), "sign-in expired"),
            (fixtures::reconnecting(), "connection lost, retrying"),
        ];
        for (data, title) in cases {
            assert_eq!(at(&data, 80).trim_end(), row(title), "{title}");
        }
    }

    #[test]
    fn the_header_while_the_badges_are_read_and_when_nothing_is_left() {
        let reading = fixtures::reading_badges();
        assert_eq!(at(&reading, 80).trim_end(), row("reading the badges (S)"));
        assert_eq!(at(&reading, 60).trim_end(), row("reading the badges (XS)"));
        let idle = fixtures::nothing_to_farm();
        assert_eq!(
            at(&idle, 120).trim_end(),
            row("nothing left to farm, with the session's summary")
        );
    }

    #[test]
    fn a_narrow_window_keeps_the_pill_and_the_state() {
        let data = fixtures::farming_alone();
        assert_eq!(
            at(&data, 49),
            " steamcards   ● farming      appears offline  [?]"
        );
        assert_eq!(at(&data, 10), " steamcards ");
    }
}
