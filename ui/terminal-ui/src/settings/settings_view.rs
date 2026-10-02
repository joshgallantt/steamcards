//! The settings pop-up: a row for each setting, the chosen one's meaning
//! under them.

use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{
    app::Ctx,
    popup::{dim, fit_height, modal},
    theme::{self, BUSY, GOOD},
    widgets::{hints, keycap, scroll, selected_row, spread, wrap_text},
};

#[derive(Debug, Default)]
pub(crate) struct SettingsView {
    pub(crate) cursor: usize,
}

/// The settings, in the order they're listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Setting {
    HoursBeforeDrops,
    SkipPrivate,
    SkipRefundable,
    RestartGames,
    AppearOffline,
    AutoUpdate,
}

impl Setting {
    pub(crate) const ALL: [Setting; 6] = [
        Setting::HoursBeforeDrops,
        Setting::SkipPrivate,
        Setting::SkipRefundable,
        Setting::RestartGames,
        Setting::AppearOffline,
        Setting::AutoUpdate,
    ];

    /// The setting on a row; past the last, the last.
    pub(crate) fn at(row: usize) -> Setting {
        Self::ALL[row.min(Self::ALL.len() - 1)]
    }

    /// The row it's on.
    pub(crate) fn row(self) -> usize {
        Self::ALL
            .iter()
            .position(|&s| s == self)
            .unwrap_or_default()
    }

    /// The key that changes it from anywhere in the pop-up. The hours have
    /// the arrows.
    pub(crate) fn key(self) -> Option<char> {
        match self {
            Setting::HoursBeforeDrops => None,
            Setting::SkipPrivate => Some('p'),
            Setting::SkipRefundable => Some('b'),
            Setting::RestartGames => Some('r'),
            Setting::AppearOffline => Some('v'),
            Setting::AutoUpdate => Some('u'),
        }
    }

    /// The setting `key` changes, if any.
    pub(crate) fn keyed(key: char) -> Option<Setting> {
        Self::ALL.into_iter().find(|s| s.key() == Some(key))
    }

    fn label(self) -> &'static str {
        match self {
            Setting::HoursBeforeDrops => "Hours before cards drop",
            Setting::SkipPrivate => "Skip private games",
            Setting::SkipRefundable => "Skip recently bought games",
            Setting::RestartGames => "Restart the game every 5 minutes",
            Setting::AppearOffline => "Appear offline while farming",
            Setting::AutoUpdate => "Keep steamcards up to date",
        }
    }

    /// Whether it's on, for a setting that's on or off.
    fn on(self, cx: &Ctx<'_>) -> Option<bool> {
        let p = cx.prefs;
        match self {
            Setting::HoursBeforeDrops => None,
            Setting::SkipPrivate => Some(p.skip_private),
            Setting::SkipRefundable => Some(p.skip_refundable),
            Setting::RestartGames => Some(p.restart_games),
            Setting::AppearOffline => Some(!p.appear_online),
            Setting::AutoUpdate => Some(p.auto_update),
        }
    }

    /// What it does, as it's set now.
    fn about(self, cx: &Ctx<'_>) -> String {
        let p = cx.prefs;
        match self {
            Setting::HoursBeforeDrops => "Steam drops a game's cards once it has this many hours \
                                          on record: 3 on most accounts. Games short of that play \
                                          together, up to 32, to build them first. 0 farms every \
                                          game on its own, for an account Steam doesn't hold back."
                .into(),
            Setting::SkipPrivate => "Steam drops no cards for games marked private in your Steam \
                                     library, so farming them would take hours for nothing."
                .into(),
            Setting::SkipRefundable => "Games bought in the last 14 days, played under 2 hours, \
                                        wait until Steam won't refund them, so farming doesn't \
                                        cost you the refund."
                .into(),
            Setting::RestartGames => "Stops the game being farmed every 5 minutes, and plays it \
                                      again a moment later, to shake drops loose."
                .into(),
            Setting::AppearOffline if p.appear_online => {
                "Friends see you online, and every game being played.".into()
            }
            Setting::AppearOffline => {
                "Friends don't see the games being played. Steam counts them just the same.".into()
            }
            Setting::AutoUpdate => {
                let version = env!("CARGO_PKG_VERSION");
                match (p.auto_update, cx.app.updates.last()) {
                    (true, Some(found)) => farming_words::update(&found),
                    (true, None) => format!(
                        "This is steamcards {version}. It looks for a new release once a day."
                    ),
                    (false, _) => format!(
                        "This is steamcards {version}. It asks GitHub nothing: update it yourself."
                    ),
                }
            }
        }
    }
}

pub(crate) fn render(f: &mut Frame<'_>, area: Rect, cx: &Ctx<'_>, v: &SettingsView) {
    // As wide as it can be, up to 66 columns: what's in it wraps to fit.
    let w = 66.min(area.width);
    let inner_w = w.saturating_sub(6) as usize;
    let chosen = Setting::at(v.cursor);
    let mut lines = Vec::new();
    let mut cursor_line = 0;
    for s in Setting::ALL {
        // Farming's, then how it shows and keeps up to date.
        if s == Setting::AppearOffline {
            lines.push(Line::default());
        }
        let mut left = vec![Span::raw(" ")];
        left.push(match s.on(cx) {
            Some(true) => Span::styled(format!("{} ", theme::RADIO_ON), theme::strong(GOOD)),
            Some(false) => Span::styled(format!("{} ", theme::RADIO_OFF), theme::dim()),
            None => Span::raw("  "),
        });
        left.push(Span::raw(s.label()));
        let right = match s.key() {
            Some(key) => vec![keycap(&key.to_string()), Span::raw(" ")],
            None => vec![
                dim("‹ "),
                Span::styled(cx.prefs.hours_before_drops.to_string(), theme::strong(BUSY)),
                dim(" › "),
            ],
        };
        let row = spread(left, right, inner_w);
        if s == chosen {
            cursor_line = lines.len();
            lines.push(selected_row(row, inner_w));
        } else {
            lines.push(row);
        }
    }
    lines.push(Line::default());
    lines.extend(
        wrap_text(&chosen.about(cx), inner_w.saturating_sub(1))
            .into_iter()
            .map(|l| Line::styled(format!(" {l}"), theme::dim())),
    );

    let keys = hints(
        &[
            ("↑↓", "choose", 2),
            ("space", "change", 1),
            ("←→", "hours", 3),
            ("esc", "close", 0),
        ],
        inner_w,
    );
    let h = fit_height(lines.len()).min(area.height.saturating_sub(2));
    let inner = modal(f, area, w, h, "Settings", keys);
    let off = scroll(cursor_line, lines.len(), inner.height as usize);
    f.render_widget(Paragraph::new(lines).scroll((off as u16, 0)), inner);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_but_the_hours_has_a_key_of_its_own() {
        for s in Setting::ALL {
            assert_eq!(Setting::at(s.row()), s);
            match s.key() {
                Some(key) => assert_eq!(Setting::keyed(key), Some(s)),
                None => assert_eq!(s, Setting::HoursBeforeDrops),
            }
        }
        assert_eq!(Setting::at(99), Setting::AutoUpdate, "past the last row");
        assert_eq!(Setting::keyed('s'), None, "s closes it");
    }
}
