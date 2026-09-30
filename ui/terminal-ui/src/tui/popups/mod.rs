// The pop-ups (docs/design/ui.md §2.2, §2.5): a game's details, this
// session's cards, the market, games & settings, the account and its
// sign-out question, signing in, the log, help, and quitting.
//
// A pop-up covers whole panels, never part of one: the right-hand column
// (below Now, at L) where the spec's table puts it and the size has one, or
// else the whole body between the header and the strip, which stay in view
// with the footer. While it has the keys, what can still be seen of the
// screen, and the footer, are faded. Its keys go in its bottom border when
// they all fit there, or on its last lines when they don't; text longer
// than its area scrolls, and the border says how much is below.

mod account;
mod details;
mod games;
mod haul;
mod help;
mod log;
mod market;
mod quit;
mod sign_in;

#[cfg(test)]
mod tests;

use std::time::Instant;

use ::account::LoginChallenge;
use crossterm::event::{KeyCode, KeyEvent};
use preferences::Tier;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
};

use super::{
    App, Ctx,
    layout::{Popup, place, popup_area},
    moved,
    text::{Fits, Hint, Titles, panel, put_lines},
    theme,
};
use crate::viewmodel::{MarketView, game_market_page, market_page, open_in_browser};

pub(super) use games::GamesView;
pub(super) use haul::HaulView;
pub(super) use log::LogView;
pub(super) use market::MarketList;

/// The pop-up open over the screen, and where each is at.
pub(super) enum Overlay {
    /// The chosen game's details.
    Details(Scroll),
    /// This session's cards.
    Haul(HaulView),
    /// The market: where every price comes from.
    Market(MarketList),
    Games(GamesView),
    Account {
        /// Asking whether to sign out.
        confirm: bool,
    },
    Login(LoginView),
    Log(LogView),
    Help(Scroll),
    ConfirmQuit,
}

/// Signing in with a QR code, and how it went.
pub(super) struct LoginView {
    pub(super) challenge: Option<LoginChallenge>,
    /// `Ok(account name)` or `Err(reason)` once the sign-in ends.
    pub(super) outcome: Option<Result<String, String>>,
    pub(super) finished: Option<Instant>,
    /// Return to the account pop-up (rather than the dashboard) afterwards.
    pub(super) from_account: bool,
}

/// How far a pop-up is scrolled, and how far it can go, as last drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Scroll {
    pub(super) offset: usize,
    /// The lines a page shows.
    page: usize,
    /// The furthest it scrolls.
    max: usize,
}

impl Scroll {
    /// Scrolls for a key that scrolls: a line for ↑↓ when `by_line`, a page
    /// for PgUp PgDn, to either end for Home End.
    fn key(&mut self, code: KeyCode, by_line: bool) {
        let page = self.page.max(1);
        let at = self.offset.min(self.max);
        self.offset = match code {
            KeyCode::Up if by_line => at.saturating_sub(1),
            KeyCode::Down if by_line => (at + 1).min(self.max),
            KeyCode::PageUp => at.saturating_sub(page),
            KeyCode::PageDown => (at + page).min(self.max),
            KeyCode::Home => 0,
            KeyCode::End => self.max,
            _ => at,
        };
    }

    /// Scrolled so line `focus` shows, `page` lines at a time.
    fn keep(&mut self, focus: usize, page: usize) {
        if focus < self.offset {
            self.offset = focus;
        } else if page > 0 && focus >= self.offset + page {
            self.offset = focus + 1 - page;
        }
    }
}

/// What a pop-up says: its title and a note in its top border, its lines,
/// and its keys.
struct Shown {
    title: Line<'static>,
    note: Option<Line<'static>>,
    lines: Vec<Line<'static>>,
    keys: Vec<Hint>,
    /// The line to keep in view, a cursor's.
    focus: Option<usize>,
}

impl Shown {
    fn new(title: &str, lines: Vec<Line<'static>>, keys: &[Hint]) -> Self {
        Self {
            title: Line::styled(title.to_owned(), theme::heading()),
            note: None,
            lines,
            keys: keys.to_vec(),
            focus: None,
        }
    }
}

// ── Drawing ──────────────────────────────────────────────────────────────────

/// Draws the open pop-up over the screen, and fades what's left of it.
pub(super) fn render(buf: &mut Buffer, cx: &Ctx<'_>, overlay: &mut Overlay) {
    let kind = match overlay {
        Overlay::Details(_) => Popup::Details,
        Overlay::Account { .. } => Popup::Account,
        Overlay::Login(_) => Popup::SignIn,
        Overlay::ConfirmQuit => Popup::Quit,
        _ => Popup::Wide,
    };
    // Onboarding has no panels: a pop-up takes its whole body.
    let area = if cx.app.onboarding.is_active() {
        cx.regions.body
    } else {
        popup_area(kind, &cx.regions)
    };
    let faded = Style::new().add_modifier(Modifier::DIM);
    buf.set_style(cx.regions.body, faded);
    buf.set_style(cx.regions.footer, faded);
    clear(buf, area);
    let drawn = match overlay {
        Overlay::Details(scroll) => {
            details::shown(cx, area).and_then(|s| show(buf, area, s, scroll))
        }
        Overlay::Haul(v) => {
            haul::shown(cx, area, v).and_then(|s| show(buf, area, s, &mut v.scroll))
        }
        Overlay::Market(v) => {
            market::shown(cx, area, v).and_then(|s| show(buf, area, s, &mut v.scroll))
        }
        Overlay::Games(v) => {
            games::shown(cx, area, v).and_then(|s| show(buf, area, s, &mut v.scroll))
        }
        Overlay::Account { confirm } => account::shown(cx, area, *confirm)
            .and_then(|s| show(buf, area, s, &mut Scroll::default())),
        Overlay::Login(v) => {
            sign_in::shown(cx, area, v).and_then(|s| show(buf, area, s, &mut Scroll::default()))
        }
        Overlay::Log(v) => log::draw(buf, cx, area, v),
        Overlay::Help(scroll) => help::shown(cx, area).and_then(|s| show(buf, area, s, scroll)),
        Overlay::ConfirmQuit => {
            quit::shown(area).and_then(|s| show(buf, area, s, &mut Scroll::default()))
        }
    };
    if let Err(e) = drawn {
        // A ladder let something through: the sweep catches it in the tests.
        // The running app, which must keep farming, shows the empty frame.
        if cfg!(test) {
            panic!("a pop-up over {area:?} didn't fit: {e}");
        }
        clear(buf, area);
        frame(buf, area, Line::default(), None, None, &[]);
    }
}

/// Blanks `area`, fading and all, for a pop-up to be drawn on.
fn clear(buf: &mut Buffer, area: Rect) {
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buf[(x, y)].reset();
        }
    }
}

/// Draws `shown` over `area`, scrolled as `scroll` says, keeping its cursor
/// in view, and notes how far it can scroll.
fn show(buf: &mut Buffer, area: Rect, shown: Shown, scroll: &mut Scroll) -> Fits<()> {
    if let Some(focus) = shown.focus {
        let page = place(area, &shown.lines, &shown.keys, 0)?.page;
        scroll.keep(focus, page);
    }
    let placed = place(area, &shown.lines, &shown.keys, scroll.offset)?;
    scroll.offset = placed.offset;
    scroll.page = placed.page;
    scroll.max = shown.lines.len().saturating_sub(placed.page);
    frame(
        buf,
        area,
        shown.title,
        shown.note,
        placed.keys,
        &placed.lines,
    );
    Ok(())
}

/// A pop-up's frame over `area`: its border in the pop-ups' colour, its
/// title top left and its note top right when there's room for both, what
/// its bottom border says, and its lines, 2 columns in from each side.
fn frame(
    buf: &mut Buffer,
    area: Rect,
    title: Line<'static>,
    note: Option<Line<'static>>,
    bottom: Option<Line<'static>>,
    lines: &[Line<'static>],
) {
    let w = usize::from(area.width);
    let note = note.filter(|n| n.width() > 0 && n.width() + title.width() + 11 <= w);
    let title = (title.width() > 0).then_some(title);
    panel(
        buf,
        area,
        theme::modal_border(),
        &Titles {
            top_left: title,
            top_right: note,
            bottom_left: None,
            bottom_right: bottom.filter(|b| b.width() > 0),
        },
    );
    let inside = Rect::new(
        area.x + 3,
        area.y + 1,
        area.width.saturating_sub(6),
        area.height.saturating_sub(2),
    );
    put_lines(buf, inside, lines);
}

/// A row as the selection shows it: the whole row in the selection's bar.
fn chosen(line: Line<'static>) -> Line<'static> {
    let mut line = line;
    for span in &mut line.spans {
        span.style = theme::selected();
    }
    line.style(theme::selected())
}

/// Whether a key moves a cursor: ↑↓, PgUp PgDn, Home End.
fn moves(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Home
            | KeyCode::End
    )
}

/// A game's name in a cell `w` wide: whole, or shortened at a word
/// boundary; blank rather than cut when not even its first word fits.
fn game_name(name: &str, w: usize) -> String {
    super::text::shorten(name, w).unwrap_or_default()
}

/// Items in a sentence: "A", "A and B" (or "A, and B", two clauses), "A, B,
/// and C".
fn listed(items: &[String], comma_before_and: bool) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [a, b] if !comma_before_and => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

// ── Keys ─────────────────────────────────────────────────────────────────────

impl App {
    /// Handles a key for the open pop-up; returns what should be open next.
    pub(super) fn overlay_key(&mut self, o: Overlay, k: KeyEvent) -> Option<Overlay> {
        match o {
            Overlay::Help(mut scroll) => match k.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?' | 'q') => None,
                code => {
                    scroll.key(code, true);
                    Some(Overlay::Help(scroll))
                }
            },

            Overlay::ConfirmQuit => match k.code {
                KeyCode::Char('y' | 'q') | KeyCode::Enter => {
                    self.quit = true;
                    None
                }
                KeyCode::Char('n') | KeyCode::Esc => None,
                _ => Some(Overlay::ConfirmQuit),
            },

            Overlay::Account { confirm: true } => {
                // Only y signs out; anything else keeps the sign-in.
                if k.code == KeyCode::Char('y') {
                    self.sign_out();
                }
                // Signed out, onboarding has taken over.
                (!self.onboarding.is_active()).then_some(Overlay::Account { confirm: false })
            }

            Overlay::Account { .. } => {
                let signed_in = self.account.is_signed_in();
                match k.code {
                    KeyCode::Enter => Some(self.begin_login(true)),
                    KeyCode::Char('d') if signed_in => Some(Overlay::Account { confirm: true }),
                    KeyCode::Char('v') => {
                        self.toggle_appear_online();
                        Some(Overlay::Account { confirm: false })
                    }
                    KeyCode::Esc | KeyCode::Char('a' | 'q') => None,
                    _ => Some(Overlay::Account { confirm: false }),
                }
            }

            Overlay::Login(v) => self.login_key(v, k),
            Overlay::Games(v) => self.games_key(v, k),
            Overlay::Log(v) => self.log_key(v, k),
            Overlay::Details(scroll) => self.details_key(scroll, k),
            Overlay::Haul(v) => self.haul_key(v, k),
            Overlay::Market(v) => self.market_key(v, k),
        }
    }

    fn details_key(&mut self, mut scroll: Scroll, k: KeyEvent) -> Option<Overlay> {
        match k.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return None,
            // The next game, without closing: its details from the top.
            KeyCode::Up => {
                self.move_selection(-1);
                return self.open_details();
            }
            KeyCode::Down => {
                self.move_selection(1);
                return self.open_details();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.set_tier(Tier::Priority(c as usize - '0' as usize));
            }
            KeyCode::Char('0') | KeyCode::Backspace | KeyCode::Delete => {
                self.set_tier(Tier::Indifferent);
            }
            KeyCode::Char('x') => self.set_tier(Tier::Skip),
            KeyCode::Char('o') => self.open_card_page(),
            code => scroll.key(code, false),
        }
        Some(Overlay::Details(scroll))
    }

    fn haul_key(&mut self, mut v: HaulView, k: KeyEvent) -> Option<Overlay> {
        let drops = self.dropped() as usize;
        match k.code {
            KeyCode::Esc | KeyCode::Char('h' | 'q') => return None,
            KeyCode::Char('b') => self.next_basis(),
            KeyCode::Char('o') => self.open_market_page(v.card),
            code if moves(code) => {
                let at = v.card.unwrap_or(drops.saturating_sub(1));
                v.card = Some(moved(at, code, drops));
            }
            // Quick-sell's keys, s, k and u among them, are kept for it.
            _ => {}
        }
        Some(Overlay::Haul(v))
    }

    /// Opens the market page of the card that dropped `card`th (the newest
    /// when `None`), once which card it was is known.
    fn open_market_page(&mut self, card: Option<usize>) {
        let Some(drops) = self.status.as_ref().map(|s| &s.session.drops) else {
            return;
        };
        let Some(drop) = card.map_or(drops.last(), |i| drops.get(i)) else {
            return;
        };
        match &drop.card {
            farming::DropCard::Identified(asset) => {
                let url = market_page(&asset.market_hash_name);
                match open_in_browser(&url) {
                    Ok(()) => self.flash(format!("Opened {}'s market page.", asset.name)),
                    Err(err) => self.flash(format!("Couldn't open a browser ({err}). Visit {url}")),
                }
            }
            farming::DropCard::NameOnly { name, .. } => self.flash(format!(
                "{name} was told by its card page, which gives no market page."
            )),
            farming::DropCard::Identifying => {
                self.flash("Which card it was is still being found out.");
            }
            farming::DropCard::Unknown => self.flash("Which card it was couldn't be told."),
        }
    }

    fn market_key(&mut self, mut v: MarketList, k: KeyEvent) -> Option<Overlay> {
        let (games, view) = {
            let known = self.known();
            let view = MarketView::build(&self.snapshot(&known), self.market.paused_since());
            (market::games(&view), view)
        };
        let at = market::cursor(&games, v.game, self.selected);
        let game = games.get(at).copied();
        match k.code {
            KeyCode::Esc | KeyCode::Char('m' | 'q') => return None,
            KeyCode::Char('b') => self.next_basis(),
            KeyCode::Char('o') => {
                if let Some(app_id) = game {
                    let name = self
                        .known_library()
                        .game(app_id)
                        .map_or_else(|| format!("App {app_id}"), |g| g.name.clone());
                    let url = game_market_page(app_id);
                    match open_in_browser(&url) {
                        Ok(()) => self.flash(format!("Opened {name}'s cards on the market.")),
                        Err(err) => {
                            self.flash(format!("Couldn't open a browser ({err}). Visit {url}"));
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                if let Some(app_id) = game {
                    self.look_again(app_id, &view);
                }
            }
            code if moves(code) => v.game = games.get(moved(at, code, games.len())).copied(),
            // Quick-sell's keys, tab, space and ←→ among them, are kept for
            // it.
            _ => {}
        }
        Some(Overlay::Market(v))
    }

    /// Asks the market to look at a game's prices again: queued, and after
    /// Steam's pause if there is one.
    fn look_again(&mut self, app_id: u32, view: &MarketView) {
        let name = self
            .known_library()
            .game(app_id)
            .map_or_else(|| format!("App {app_id}"), |g| g.name.clone());
        self.market.refresh(app_id);
        match view.pause {
            Some(p) => {
                let until = super::format::clock(p.until, view.now, view.zone);
                self.flash(format!(
                    "{name}'s prices are looked up again once Steam's pause ends, at {until}."
                ));
            }
            None => self.flash(format!(
                "{name}'s prices are looked up again, unless they're under an hour old."
            )),
        }
    }

    fn log_key(&mut self, mut v: LogView, k: KeyEvent) -> Option<Overlay> {
        match k.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('l' | 'q') => return None,
            code => v.key(code, self.log.len()),
        }
        Some(Overlay::Log(v))
    }

    fn login_key(&mut self, v: LoginView, k: KeyEvent) -> Option<Overlay> {
        match (&v.outcome, k.code) {
            (Some(Err(_)), KeyCode::Char('r')) => Some(self.begin_login(v.from_account)),
            (Some(_), KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) => {
                self.after_login(v.from_account)
            }
            (None, KeyCode::Esc) => {
                self.flash("Stopped signing in.");
                self.after_login(v.from_account)
            }
            _ => Some(Overlay::Login(v)),
        }
    }

    fn games_key(&mut self, mut v: GamesView, k: KeyEvent) -> Option<Overlay> {
        let rows = |app: &Self| app.games.rows(app.known_library().games());
        // Keeps the cursor on a game that moved, e.g. to its new rank.
        let follow = |app: &Self, app_id: u32| rows(app).iter().position(|x| x.app_id == app_id);
        let all = rows(self);
        let row = all.get(v.cursor.min(all.len().saturating_sub(1))).cloned();
        // Space picks; enter, as in every pop-up, closes.
        match k.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('g' | 'q') => return None,
            KeyCode::Char(' ') => {
                if let Some(r) = row {
                    self.toggle_game(r.app_id, &r.name);
                    v.cursor = follow(self, r.app_id).unwrap_or(v.cursor);
                }
            }
            KeyCode::Char(c @ '1'..='9') => {
                if let Some(r) = row {
                    let n = c as usize - '0' as usize;
                    match self.games.set_tier(r.app_id, Tier::Priority(n)) {
                        Ok(()) => {
                            v.cursor = follow(self, r.app_id).unwrap_or(0);
                            let rank = self
                                .games
                                .rows(self.known_library().games())
                                .iter()
                                .find(|x| x.app_id == r.app_id)
                                .and_then(|x| x.rank)
                                .unwrap_or(n);
                            self.flash(format!("{} is priority #{rank}: farmed first.", r.name));
                        }
                        Err(e) => self.didnt_stick(e),
                    }
                }
            }
            KeyCode::Char('0') | KeyCode::Backspace | KeyCode::Delete => {
                if let Some(r) = row.filter(|r| r.rank.is_some()) {
                    match self.games.set_tier(r.app_id, Tier::Indifferent) {
                        Ok(()) => self.flash(format!("{} is no longer a priority game.", r.name)),
                        Err(e) => self.didnt_stick(e),
                    }
                }
            }
            KeyCode::Char('o') => self.toggle_only_priority(),
            KeyCode::Char('r') => self.library.refresh(),
            KeyCode::Char('v') => self.toggle_appear_online(),
            KeyCode::Char('b') => self.next_basis(),
            code => v.cursor = moved(v.cursor, code, all.len()),
        }
        Some(Overlay::Games(v))
    }
}
