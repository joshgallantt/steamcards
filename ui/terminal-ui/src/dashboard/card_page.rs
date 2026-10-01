use steam_library::AppId;

/// Opens a page in the default browser: through AppKit on macOS, and through
/// `$BROWSER` or the desktop's opener (`xdg-open` and the like) on Linux. A
/// graphical browser's output never reaches the TUI; a text one, like lynx,
/// has the terminal until it quits.
pub fn open_in_browser(url: &str) -> std::io::Result<()> {
    webbrowser::open(url)
}

/// A game's card page on steamcommunity.com, for the signed-in account.
pub fn card_page(app_id: AppId) -> String {
    format!("https://steamcommunity.com/my/gamecards/{app_id}/")
}
