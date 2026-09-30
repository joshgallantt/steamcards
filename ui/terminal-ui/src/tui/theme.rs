// Colours, styles and symbols in one place. Only named ANSI colours, so the UI
// follows the terminal's own palette in light and dark themes alike.

use ratatui::style::{Color, Modifier, Style};

pub(super) const ACCENT: Color = Color::Magenta;
/// Reserved for "the thing you've selected" — the cursor bar, the details
/// panel it's shown in, and the line joining them. Nothing else is blue.
pub(super) const SELECT: Color = Color::LightBlue;
pub(super) const GOOD: Color = Color::Green;
pub(super) const BUSY: Color = Color::Yellow;
pub(super) const BAD: Color = Color::Red;
pub(super) const LINK: Color = Color::Cyan;
pub(super) const DIM: Color = Color::DarkGray;

pub(super) const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub(super) fn dim() -> Style {
    Style::new().fg(DIM)
}

pub(super) fn fg(c: Color) -> Style {
    Style::new().fg(c)
}

pub(super) fn bold() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

pub(super) fn strong(c: Color) -> Style {
    Style::new().fg(c).add_modifier(Modifier::BOLD)
}

/// A key drawn like a keyboard key: dark letter on a light cap, which holds
/// its contrast in light and dark themes alike.
pub(super) fn keycap() -> Style {
    Style::new().fg(Color::Black).bg(Color::Gray)
}

/// The selection bar: the whole row painted in the selection colour. Drawn
/// reversed, so its text takes the terminal's own background colour — dark on
/// dark themes, light on light ones — and stays readable in both.
pub(super) fn selected() -> Style {
    Style::new()
        .fg(SELECT)
        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

pub(super) fn brand() -> Style {
    Style::new().fg(ACCENT).add_modifier(Modifier::BOLD)
}

/// The app's name as a pill: " steamcards " in reverse.
pub(super) fn pill() -> Style {
    Style::new()
        .fg(ACCENT)
        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
}

/// Headings and panel titles. The explicit default colour stops titles from
/// inheriting their panel's dim border colour.
pub(super) fn heading() -> Style {
    Style::new().fg(Color::Reset).add_modifier(Modifier::BOLD)
}

/// Steam's own name, wherever the account is shown.
pub(super) fn steam() -> Style {
    strong(LINK)
}

pub(super) fn border() -> Style {
    Style::new().fg(DIM)
}

pub(super) fn modal_border() -> Style {
    Style::new().fg(ACCENT)
}
