//! `cargo xtask screenshots`: the README's images of the TUI.
//!
//! The preview tests draw every screen into an in-memory terminal with
//! made-up data, and with `PREVIEW_DUMP` set they write each screen's cells as
//! JSON. This turns the chosen screens into SVG, so the images:
//! - are the same on every OS;
//! - never show a real account;
//! - are regenerated with one command after any UI change.

use std::{collections::HashMap, fmt::Write as _, fs, path::Path, process::Command};

use serde_json::Value;

/// The README's screens: preview name → image file (in `docs/images`).
const SCREENS: &[(&str, &str)] = &[
    ("readme: dashboard 146×31", "dashboard"),
    ("readme: a game's details 120×34", "details"),
    ("sign-in QR 120×32", "sign-in"),
];

/// Cell size in pixels, for a 14 px monospace font.
const CELL_W: f64 = 8.4;
const CELL_H: f64 = 17.0;
const FONT_SIZE: f64 = 14.0;
/// Where a line's baseline sits within its cell.
const BASELINE: f64 = 13.0;
/// Space around the terminal, and the window's title bar.
const PAD: f64 = 14.0;
const TITLE_BAR: f64 = 30.0;

const BACKGROUND: &str = "#1a1b26";
const FOREGROUND: &str = "#c0caf5";
const TITLE_BAR_FILL: &str = "#16161e";

/// Runs the previews with dumps on, and writes each chosen screen as SVG.
/// With `--check`, writes nothing, and fails if an image in `docs/images`
/// isn't what the UI draws now: CI's way of keeping the README honest.
pub(crate) fn run(root: &Path, args: &[String]) -> Result<(), String> {
    let check = match args {
        [] => false,
        [flag] if flag == "--check" => true,
        _ => return Err("usage: cargo xtask screenshots [--check]".into()),
    };
    let dump = std::env::temp_dir().join(format!("steamcards-previews-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dump);
    fs::create_dir_all(&dump).map_err(|e| format!("{}: {e}", dump.display()))?;

    let rendered = Command::new(crate::CARGO)
        .args(["test", "--quiet", "-p", "terminal-ui", "previews"])
        .env("PREVIEW_DUMP", &dump)
        .env_remove("CARGO_PKG_NAME")
        .current_dir(root)
        .status()
        .is_ok_and(|s| s.success());
    if !rendered {
        return Err("the preview tests failed".into());
    }

    let mut screens: HashMap<String, Value> = HashMap::new();
    for entry in fs::read_dir(&dump).map_err(|e| e.to_string())?.flatten() {
        let text = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
        let screen: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if let Some(name) = screen["name"].as_str() {
            screens.insert(name.to_owned(), screen);
        }
    }
    let _ = fs::remove_dir_all(&dump);

    let images = root.join("docs").join("images");
    if !check {
        fs::create_dir_all(&images).map_err(|e| e.to_string())?;
    }
    let mut stale = Vec::new();
    for (preview, file) in SCREENS {
        let screen = screens.get(*preview).ok_or_else(|| {
            format!("no preview named {preview:?}; see ui/terminal-ui/src/app/preview.rs")
        })?;
        let svg = render(screen)?;
        let path = images.join(format!("{file}.svg"));
        if check {
            if fs::read_to_string(&path).ok().as_deref() != Some(svg.as_str()) {
                stale.push(format!("{file}.svg"));
            }
        } else {
            fs::write(&path, svg).map_err(|e| format!("{}: {e}", path.display()))?;
            println!("✓ {}", path.display());
        }
    }
    if !stale.is_empty() {
        return Err(format!(
            "the README's screenshots don't match the UI any more ({}). Run `cargo xtask \
             screenshots`, and commit docs/images.",
            stale.join(", ")
        ));
    }
    if check {
        println!("✓ The README's screenshots match the UI.");
    }
    Ok(())
}

/// One terminal cell, as the preview dump records it.
struct Cell<'a> {
    symbol: &'a str,
    fg: &'a str,
    bg: &'a str,
    bold: bool,
    reversed: bool,
    dim: bool,
}

impl<'a> Cell<'a> {
    fn read(v: &'a Value) -> Option<Self> {
        Some(Self {
            symbol: v.get(0)?.as_str()?,
            fg: v.get(1)?.as_str()?,
            bg: v.get(2)?.as_str()?,
            bold: v.get(3)?.as_bool()?,
            reversed: v.get(4)?.as_bool()?,
            dim: v.get(5)?.as_bool()?,
        })
    }

    /// Foreground and background as drawn, after `reversed`.
    fn colours(&self) -> (String, String) {
        let fg = colour(self.fg, FOREGROUND);
        let bg = colour(self.bg, BACKGROUND);
        if self.reversed { (bg, fg) } else { (fg, bg) }
    }
}

/// A screen dump as an SVG image in a window frame.
fn render(screen: &Value) -> Result<String, String> {
    let grid: Vec<Vec<Cell<'_>>> = screen["cells"]
        .as_array()
        .ok_or("no cells in the dump")?
        .iter()
        .map(|row| {
            row.as_array()?
                .iter()
                .map(Cell::read)
                .collect::<Option<Vec<_>>>()
        })
        .collect::<Option<_>>()
        .ok_or("a cell couldn't be read")?;
    let width = grid.first().map_or(0, Vec::len);
    let w = width as f64 * CELL_W + 2.0 * PAD;
    let h = grid.len() as f64 * CELL_H + 2.0 * PAD + TITLE_BAR;

    let mut svg = String::new();
    let _ = write!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.0}" height="{h:.0}" viewBox="0 0 {w:.1} {h:.1}" font-family="ui-monospace, SFMono-Regular, Menlo, Consolas, 'DejaVu Sans Mono', monospace" font-size="{FONT_SIZE}" xml:space="preserve">"#
    );
    let _ = write!(
        svg,
        r#"<rect width="{w:.1}" height="{h:.1}" rx="10" fill="{BACKGROUND}"/><path d="M0 10a10 10 0 0 1 10-10h{:.1}a10 10 0 0 1 10 10v{:.1}h-{w:.1}z" fill="{TITLE_BAR_FILL}"/>"#,
        w - 20.0,
        TITLE_BAR - 10.0
    );
    for (i, dot) in ["#ff5f56", "#ffbd2e", "#27c93f"].iter().enumerate() {
        let _ = write!(
            svg,
            r#"<circle cx="{:.1}" cy="{:.1}" r="6" fill="{dot}"/>"#,
            PAD + 6.0 + i as f64 * 20.0,
            TITLE_BAR / 2.0
        );
    }
    let _ = write!(
        svg,
        r##"<text x="{:.1}" y="{:.1}" text-anchor="middle" fill="#565f89">steamcards</text>"##,
        w / 2.0,
        TITLE_BAR / 2.0 + 5.0
    );

    // Pixel-snapped, so neighbouring rows' fills leave no hairlines between
    // them when the image is scaled.
    svg.push_str(r#"<g shape-rendering="crispEdges">"#);
    for (y, row) in grid.iter().enumerate() {
        backgrounds(&mut svg, row, top(y));
        blocks(&mut svg, row, top(y));
    }
    svg.push_str("</g>");
    lines(&mut svg, &grid);
    for (y, row) in grid.iter().enumerate() {
        text(&mut svg, row, top(y));
    }
    svg.push_str("</svg>\n");
    // An element a line, so a diff shows what changed on screen.
    Ok(svg.replace("><", ">\n<"))
}

/// Where row `y` starts, below the title bar.
fn top(y: usize) -> f64 {
    PAD + TITLE_BAR + y as f64 * CELL_H
}

/// Where column `x` starts.
fn left(x: usize) -> f64 {
    PAD + x as f64 * CELL_W
}

/// Background runs that differ from the terminal's.
fn backgrounds(svg: &mut String, cells: &[Cell<'_>], top: f64) {
    let mut x = 0;
    while x < cells.len() {
        let (_, bg) = cells[x].colours();
        let end = (x..cells.len())
            .find(|&i| cells[i].colours().1 != bg)
            .unwrap_or(cells.len());
        if bg != BACKGROUND {
            let _ = write!(
                svg,
                r#"<rect x="{:.1}" y="{top:.1}" width="{:.1}" height="{CELL_H}" fill="{bg}"/>"#,
                left(x),
                (end - x) as f64 * CELL_W
            );
        }
        x = end;
    }
}

/// A block character's filled part, in eighths of its cell, and how solid
/// it is.
struct Block {
    x: u8,
    y: u8,
    w: u8,
    h: u8,
    opacity: &'static str,
}

fn block(symbol: &str) -> Option<Block> {
    let (x, y, w, h, opacity) = match symbol {
        "█" => (0, 0, 8, 8, "1"),
        "▀" => (0, 0, 8, 4, "1"),
        "▄" => (0, 4, 8, 4, "1"),
        "▐" => (4, 0, 4, 8, "1"),
        "▏" => (0, 0, 1, 8, "1"),
        "▎" => (0, 0, 2, 8, "1"),
        "▍" => (0, 0, 3, 8, "1"),
        "▌" => (0, 0, 4, 8, "1"),
        "▋" => (0, 0, 5, 8, "1"),
        "▊" => (0, 0, 6, 8, "1"),
        "▉" => (0, 0, 7, 8, "1"),
        "░" => (0, 0, 8, 8, "0.25"),
        "▒" => (0, 0, 8, 8, "0.5"),
        "▓" => (0, 0, 8, 8, "0.75"),
        _ => return None,
    };
    Some(Block {
        x,
        y,
        w,
        h,
        opacity,
    })
}

/// Block characters (progress bars, QR codes) as rectangles, which fill
/// their cells exactly where a font's glyphs leave gaps. Full-width ones join
/// with the same ones beside them.
fn blocks(svg: &mut String, cells: &[Cell<'_>], top: f64) {
    let mut x = 0;
    while x < cells.len() {
        let cell = &cells[x];
        let Some(b) = block(cell.symbol) else {
            x += 1;
            continue;
        };
        let (fill, _) = cell.colours();
        let end = if b.w == 8 {
            (x + 1..cells.len())
                .find(|&i| {
                    cells[i].symbol != cell.symbol
                        || cells[i].colours().0 != fill
                        || cells[i].dim != cell.dim
                })
                .unwrap_or(cells.len())
        } else {
            x + 1
        };
        let _ = write!(
            svg,
            r#"<rect x="{:.2}" y="{:.2}" width="{:.2}" height="{:.2}" fill="{fill}" fill-opacity="{}"{}/>"#,
            left(x) + f64::from(b.x) / 8.0 * CELL_W,
            top + f64::from(b.y) / 8.0 * CELL_H,
            ((end - x - 1) as f64 + f64::from(b.w) / 8.0) * CELL_W,
            f64::from(b.h) / 8.0 * CELL_H,
            b.opacity,
            dimmed(cell.dim)
        );
        x = end;
    }
}

/// A box-drawing character's arms, as line weights (0 none, 1 light, 2
/// heavy) going up, right, down and left, and whether its corner is rounded.
fn arms(symbol: &str) -> Option<([u8; 4], bool)> {
    Some(match symbol {
        "─" => ([0, 1, 0, 1], false),
        "━" => ([0, 2, 0, 2], false),
        "│" => ([1, 0, 1, 0], false),
        "┃" => ([2, 0, 2, 0], false),
        "┌" => ([0, 1, 1, 0], false),
        "┐" => ([0, 0, 1, 1], false),
        "└" => ([1, 1, 0, 0], false),
        "┘" => ([1, 0, 0, 1], false),
        "├" => ([1, 1, 1, 0], false),
        "┤" => ([1, 0, 1, 1], false),
        "┬" => ([0, 1, 1, 1], false),
        "┴" => ([1, 1, 0, 1], false),
        "┼" => ([1, 1, 1, 1], false),
        "┏" => ([0, 2, 2, 0], false),
        "┓" => ([0, 0, 2, 2], false),
        "┗" => ([2, 2, 0, 0], false),
        "┛" => ([2, 0, 0, 2], false),
        "╭" => ([0, 1, 1, 0], true),
        "╮" => ([0, 0, 1, 1], true),
        "╰" => ([1, 1, 0, 0], true),
        "╯" => ([1, 0, 0, 1], true),
        _ => return None,
    })
}

/// A light line's width; heavy is twice it.
const LINE: f64 = 1.3;

/// A straight stretch of line along row or column `line`, from `from` to
/// `to` pixels.
struct Piece {
    vertical: bool,
    line: usize,
    from: f64,
    to: f64,
    weight: u8,
    colour: String,
    dim: bool,
}

impl Piece {
    fn continues(&self, next: &Self) -> bool {
        self.vertical == next.vertical
            && self.line == next.line
            && self.weight == next.weight
            && self.colour == next.colour
            && self.dim == next.dim
            && next.from <= self.to + 0.01
    }
}

/// Box-drawing characters as lines, which meet exactly where a font's glyphs
/// can leave gaps between rows. Straight pieces join into one line per run.
fn lines(svg: &mut String, grid: &[Vec<Cell<'_>>]) {
    let mut across = Vec::new();
    let mut down = Vec::new();
    for (y, row) in grid.iter().enumerate() {
        for (x, cell) in row.iter().enumerate() {
            let Some(([up, right, below, left_arm], round)) = arms(cell.symbol) else {
                continue;
            };
            let (colour, _) = cell.colours();
            let (x0, y0) = (left(x), top(y));
            let (cx, cy) = (x0 + CELL_W / 2.0, y0 + CELL_H / 2.0);
            if round {
                corner(svg, cell, (cx, cy), [up, right, below, left_arm]);
                continue;
            }
            let piece = |vertical, line, from, to, weight| Piece {
                vertical,
                line,
                from,
                to,
                weight,
                colour: colour.clone(),
                dim: cell.dim,
            };
            // Arms across reach half a line into the middle, to square off
            // the joint with the arms up and down.
            let joint = f64::from(up.max(below)) * LINE / 2.0;
            if left_arm > 0 {
                across.push(piece(false, y, x0, cx + joint, left_arm));
            }
            if right > 0 {
                across.push(piece(false, y, cx - joint, x0 + CELL_W, right));
            }
            if up > 0 {
                down.push((x, piece(true, x, y0, cy, up)));
            }
            if below > 0 {
                down.push((x, piece(true, x, cy, y0 + CELL_H, below)));
            }
        }
    }
    // Column by column, so pieces of one column are next to each other.
    down.sort_by_key(|(x, _)| *x);
    let pieces = across.into_iter().chain(down.into_iter().map(|(_, p)| p));
    let mut joined: Vec<Piece> = Vec::new();
    for piece in pieces {
        match joined.last_mut() {
            Some(last) if last.continues(&piece) => last.to = last.to.max(piece.to),
            _ => joined.push(piece),
        }
    }
    for p in joined {
        let width = f64::from(p.weight) * LINE;
        let (x, y, w, h) = if p.vertical {
            (
                left(p.line) + CELL_W / 2.0 - width / 2.0,
                p.from,
                width,
                p.to - p.from,
            )
        } else {
            (
                p.from,
                top(p.line) + CELL_H / 2.0 - width / 2.0,
                p.to - p.from,
                width,
            )
        };
        let _ = write!(
            svg,
            r#"<rect x="{x:.2}" y="{y:.2}" width="{w:.2}" height="{h:.2}" fill="{}"{}/>"#,
            p.colour,
            dimmed(p.dim)
        );
    }
}

/// A rounded corner: from the edge its arm across leaves by, curving round
/// the cell's middle, to the edge its arm up or down leaves by.
fn corner(
    svg: &mut String,
    cell: &Cell<'_>,
    (cx, cy): (f64, f64),
    [up, right, _, left_arm]: [u8; 4],
) {
    let across = if right > 0 { 1.0 } else { -1.0 };
    let vertical = if up > 0 { -1.0 } else { 1.0 };
    let r = CELL_W / 2.0;
    let weight = f64::from(right.max(left_arm)) * LINE;
    let (colour, _) = cell.colours();
    let _ = write!(
        svg,
        r#"<path d="M{:.2} {cy:.2}Q{cx:.2} {cy:.2} {cx:.2} {:.2}V{:.2}" fill="none" stroke="{colour}" stroke-width="{weight}"{}/>"#,
        cx + across * r,
        cy + vertical * r,
        cy + vertical * CELL_H / 2.0,
        dimmed(cell.dim)
    );
}

/// The attribute that fades a dim cell, or nothing.
fn dimmed(dim: bool) -> &'static str {
    if dim { r#" opacity="0.6""# } else { "" }
}

/// Text, in pieces of one style with at most single spaces inside. Each
/// piece starts at its own column and is spaced out to exactly its cells, so
/// columns line up whatever font the viewer has.
fn text(svg: &mut String, cells: &[Cell<'_>], top: f64) {
    let style = |c: &Cell<'_>| (c.colours().0, c.bold, c.dim);
    let drawn = |c: &Cell<'_>| block(c.symbol).is_none() && arms(c.symbol).is_none();
    let blank = |i: usize| cells.get(i).is_none_or(|c| c.symbol.trim().is_empty());
    let mut x = 0;
    while x < cells.len() {
        if blank(x) || !drawn(&cells[x]) {
            x += 1;
            continue;
        }
        let first = style(&cells[x]);
        let joins = |i: usize| cells.get(i).is_some_and(|c| drawn(c) && style(c) == first);
        let mut end = x + 1;
        while joins(end) && !(blank(end) && (blank(end + 1) || !joins(end + 1))) {
            end += 1;
        }
        let piece: String = cells[x..end].iter().map(|c| c.symbol).collect();
        let (fill, bold, dim) = first;
        let _ = write!(
            svg,
            r#"<text x="{:.1}" y="{:.1}" textLength="{:.1}" lengthAdjust="spacing" fill="{fill}"{}{}>{}</text>"#,
            left(x),
            top + BASELINE,
            (end - x) as f64 * CELL_W,
            if bold { r#" font-weight="bold""# } else { "" },
            dimmed(dim),
            escape(&piece)
        );
        x = end;
    }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A ratatui colour, as its `Debug` text, in the image's palette. `Reset`
/// means the terminal's own colour, given as `default`.
fn colour(name: &str, default: &str) -> String {
    let named = match name {
        "Reset" => default,
        "Black" => "#15161e",
        "Red" => "#f7768e",
        "Green" => "#9ece6a",
        "Yellow" => "#e0af68",
        "Blue" => "#7aa2f7",
        "Magenta" => "#bb9af7",
        "Cyan" => "#7dcfff",
        "Gray" => "#a9b1d6",
        "DarkGray" => "#565f89",
        "LightRed" => "#ff899d",
        "LightGreen" => "#b9f27c",
        "LightYellow" => "#ffc777",
        "LightBlue" => "#89b4fa",
        "LightMagenta" => "#c7a9ff",
        "LightCyan" => "#b4f9f8",
        "White" => "#e6e9f5",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_owned();
    }
    if let Some(rgb) = name.strip_prefix("Rgb(").and_then(|r| r.strip_suffix(')')) {
        let parts: Vec<u8> = rgb
            .split(',')
            .filter_map(|p| p.trim().parse().ok())
            .collect();
        if let [r, g, b] = parts[..] {
            return format!("#{r:02x}{g:02x}{b:02x}");
        }
    }
    default.to_owned()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn colours_come_from_the_palette_or_are_exact() {
        assert_eq!(colour("Reset", FOREGROUND), FOREGROUND);
        assert_eq!(colour("Green", FOREGROUND), "#9ece6a");
        assert_eq!(colour("Rgb(255, 255, 255)", FOREGROUND), "#ffffff");
        assert_eq!(
            colour("Indexed(42)", BACKGROUND),
            BACKGROUND,
            "unknown: the default"
        );
    }

    fn cell(symbol: &str, fg: &str, bg: &str) -> Value {
        json!([symbol, fg, bg, false, false, false])
    }

    fn one_row(cells: Vec<Value>) -> Value {
        json!({"name": "t", "width": cells.len(), "height": 1, "cells": [cells]})
    }

    #[test]
    fn a_screen_becomes_framed_svg() {
        let svg = render(&one_row(vec![cell("<", "Reset", "Gray")])).unwrap();

        assert!(svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"));
        assert!(svg.contains(">steamcards</text>"), "the window's title");
        assert!(svg.contains("&lt;</text>"), "escaped");
        assert!(svg.contains(r##"fill="#a9b1d6"/>"##), "the grey background");
    }

    #[test]
    fn text_is_placed_in_pieces_that_never_start_or_end_with_a_space() {
        let row = "a b  c "
            .chars()
            .map(|c| cell(&c.to_string(), "Green", "Reset"))
            .chain([cell("d", "Red", "Reset")])
            .collect();

        let svg = render(&one_row(row)).unwrap();

        let pieces: Vec<&str> = svg
            .split("</text>")
            .filter(|t| t.contains("<text"))
            .filter_map(|t| t.rsplit_once('>'))
            .map(|(_, text)| text)
            .filter(|t| *t != "steamcards")
            .collect();
        assert_eq!(
            pieces,
            ["a b", "c", "d"],
            "split at double spaces and colours"
        );
        assert!(
            svg.contains(&format!(r#"x="{:.1}" y="#, PAD + 5.0 * CELL_W)),
            "c at its own column"
        );
    }

    #[test]
    fn blocks_and_lines_are_shapes_joined_across_cells() {
        let row = vec![
            cell("█", "Magenta", "Reset"),
            cell("█", "Magenta", "Reset"),
            cell("╭", "Green", "Reset"),
            cell("─", "Green", "Reset"),
            cell("─", "Green", "Reset"),
        ];

        let svg = render(&one_row(row)).unwrap();

        assert!(
            svg.contains(&format!(
                r##"width="{:.2}" height="{CELL_H:.2}" fill="#bb9af7""##,
                2.0 * CELL_W
            )),
            "one rect for both blocks"
        );
        assert!(svg.contains(r#"<path d="M"#), "the rounded corner");
        assert!(
            svg.contains(&format!(
                r##"width="{:.2}" height="{LINE:.2}" fill="#9ece6a""##,
                2.0 * CELL_W
            )),
            "one line for both dashes"
        );
        assert!(
            !svg.contains(">█") && !svg.contains(">─"),
            "none of it as text"
        );
    }
}
