//! The app: onboarding on the first run, then one dashboard that stays up
//! the whole time, with pop-ups for the account, signing in, games, the log
//! and help. It holds every feature's view models, draws what's on screen
//! with each feature's views, and forwards keys to the view models; no
//! business rule lives here.

mod popups;
#[cfg(test)]
mod preview;

use std::{
    io,
    time::{Duration, Instant},
};

use account::Account as SignedIn;
use card::CardSets;
use chrono::{DateTime, FixedOffset, Local, Utc};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use farming::{FarmingStatus, FarmingUpdate, farm_order};
use futures::StreamExt;
use game::{AppId, SteamLibrary};
use preferences::{Preferences, PreferencesError, Tier};
use price::{PriceBook, PriceEvent, Wallet};
use ratatui::{DefaultTerminal, Frame, Terminal, backend::CrosstermBackend};
use session::Session;

use crate::{
    account::AccountViewModel,
    dashboard::{
        EventKind, FarmingViewModel, LibraryViewModel, LogEntry, MarketViewModel, Queue,
        QueueEntry, Section, card_page, dashboard_view, farming_log, log_view::LogView,
        open_in_browser, price_words, prices_wanted,
    },
    games::{GameRow, GamesViewModel, games_view::GamesView},
    onboarding::{
        NeedsAccount, OnboardingViewModel, Step,
        onboarding_view::{self, SetupView},
    },
    sign_in::{SignInUpdate, SignInViewModel, sign_in_view::SignInView},
    theme,
};

const MAX_LOG: usize = 1000;
pub(crate) const MIN_WIDTH: u16 = 60;
pub(crate) const MIN_HEIGHT: u16 = 16;
const FLASH_FOR: Duration = Duration::from_secs(4);
const LOGIN_DONE_FOR: Duration = Duration::from_millis(1500);
/// How old the library may be when a screen showing it opens.
const LIBRARY_FRESH: Duration = Duration::from_secs(10 * 60);
/// The sign-in step lists the account, then "Continue".
pub(crate) const CONTINUE_ROW: usize = 1;

/// What time it is, and the time zone times are shown in: the system's when
/// steamcards runs, and fixed ones in the previews, so a screen looks the
/// same on any machine, on any day.
#[derive(Clone, Copy)]
pub(crate) struct Clock {
    pub(crate) now: fn() -> DateTime<Utc>,
    pub(crate) zone: fn() -> FixedOffset,
}

impl Clock {
    const SYSTEM: Self = Self {
        now: Utc::now,
        zone: local_zone,
    };
}

/// The system's time zone, as it is now (it changes with daylight saving).
fn local_zone() -> FixedOffset {
    *Local::now().offset()
}

pub(crate) enum Overlay {
    Help,
    Account {
        /// Asking whether to sign out.
        confirm: bool,
    },
    SignIn(SignInView),
    Games(GamesView),
    Log(LogView),
    /// The chosen game's details: its set, and what each card is worth,
    /// scrolled this many lines down when they don't fit.
    Detail {
        scroll: usize,
    },
    ConfirmQuit,
}

/// Everything a frame needs, computed once per draw.
pub(crate) struct Ctx<'a> {
    pub(crate) app: &'a App,
    pub(crate) queue: &'a Queue,
    pub(crate) account: Option<&'a SignedIn>,
    pub(crate) prefs: &'a Preferences,
    pub(crate) now: DateTime<Utc>,
    /// The time zone times are shown in.
    pub(crate) zone: FixedOffset,
    /// The library as it's best known, and the games that will be farmed,
    /// in farm order.
    pub(crate) library: &'a SteamLibrary,
    pub(crate) order: &'a [AppId],
    /// The card sets of the games looked at, with the copies that dropped
    /// counted in.
    pub(crate) sets: &'a CardSets,
    /// This session's cards.
    pub(crate) session: &'a Session,
    /// Every price known, and the wallet whose currency they're shown in.
    pub(crate) book: &'a PriceBook,
    pub(crate) wallet: Option<&'a Wallet>,
}

impl Ctx<'_> {
    pub(crate) fn selected(&self) -> Option<&QueueEntry> {
        self.app.selected.and_then(|id| self.queue.get(id))
    }

    pub(crate) fn spinner(&self) -> &'static str {
        theme::SPINNER[self.app.tick % theme::SPINNER.len()]
    }

    pub(crate) fn signed_in(&self) -> bool {
        self.account.is_some()
    }

    pub(crate) fn expired(&self) -> bool {
        self.account.is_some_and(|a| a.expired)
    }
}

pub struct App {
    pub(crate) account: AccountViewModel,
    pub(crate) login: SignInViewModel,
    pub(crate) farming: FarmingViewModel,
    pub(crate) games: GamesViewModel,
    pub(crate) library: LibraryViewModel,
    pub(crate) onboarding: OnboardingViewModel,
    pub(crate) market: MarketViewModel,
    pub(crate) setup: SetupView,

    /// The farmer's last word on what it's doing.
    pub(crate) status: Option<FarmingStatus>,
    pub(crate) log: Vec<LogEntry>,
    pub(crate) clock: Clock,
    /// This session's drops already seen: a new one has its game priced
    /// again.
    pub(crate) seen_drops: usize,

    /// Game under the cursor, by app ID, so it stays put when the queue
    /// re-sorts.
    pub(crate) selected: Option<AppId>,
    pub(crate) queue_offset: usize,
    pub(crate) show_done: bool,
    pub(crate) paused_by_user: bool,

    pub(crate) overlay: Option<Overlay>,
    /// Short-lived confirmation shown above the footer.
    pub(crate) flash: Option<(String, Instant)>,
    pub(crate) tick: usize,
    pub(crate) quit: bool,
}

impl App {
    pub fn new(
        account: AccountViewModel,
        login: SignInViewModel,
        farming: FarmingViewModel,
        games: GamesViewModel,
        library: LibraryViewModel,
        onboarding: OnboardingViewModel,
        market: MarketViewModel,
    ) -> Self {
        Self {
            account,
            login,
            farming,
            games,
            library,
            onboarding,
            market,
            setup: SetupView::default(),
            status: None,
            log: Vec::new(),
            clock: Clock::SYSTEM,
            seen_drops: 0,
            selected: None,
            queue_offset: 0,
            show_done: false,
            paused_by_user: false,
            overlay: None,
            flash: None,
            tick: 0,
            quit: false,
        }
    }

    pub async fn run(mut self) -> anyhow::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

        if !self.onboarding.is_active() {
            // Farming starts by itself; there's nothing else to do until it does.
            self.farming.start();
        }
        let result = self.event_loop(&mut terminal).await;
        self.login.cancel();
        self.market.stop();
        self.farming.stop().await;

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        result
    }

    async fn event_loop(&mut self, terminal: &mut DefaultTerminal) -> anyhow::Result<()> {
        let mut events = EventStream::new();
        let mut ticker = tokio::time::interval(Duration::from_millis(100));
        while !self.quit {
            self.update();
            terminal.draw(|f| self.draw(f))?;
            tokio::select! {
                _ = ticker.tick() => self.tick = self.tick.wrapping_add(1),
                Some(Ok(ev)) = events.next() => self.on_event(ev),
            }
        }
        Ok(())
    }

    // ── State ────────────────────────────────────────────────────────────────

    fn update(&mut self) {
        while let Some(u) = self.login.try_recv() {
            self.on_login_update(u);
        }
        while let Some(u) = self.farming.try_recv() {
            self.on_farming_update(u);
        }
        while let Some(ev) = self.market.try_recv() {
            self.on_market_event(ev);
        }
        self.keep_prices_coming();
        self.library.poll();
        if let Some(Overlay::SignIn(v)) = &self.overlay {
            let done = matches!(v.outcome, Some(Ok(_)))
                && v.finished.is_some_and(|t| t.elapsed() >= LOGIN_DONE_FOR);
            if done {
                self.overlay = self.after_login(v.from_account);
            }
        }
        if self
            .flash
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= FLASH_FOR)
        {
            self.flash = None;
        }
        self.sync_selection();
    }

    /// Where farming stands, for the dashboard; what happened, in the log,
    /// in farming-words' words.
    fn on_farming_update(&mut self, u: FarmingUpdate) {
        match u {
            FarmingUpdate::Status(s) => self.status = Some(*s),
            FarmingUpdate::Event(e) => {
                let (kind, text) = farming_log::line(&e);
                self.push_log(kind, text);
            }
        }
    }

    /// What the pricing reported, in the log, in the dashboard's words.
    fn on_market_event(&mut self, ev: PriceEvent) {
        let (kind, text) = price_words::line(&ev, &self.known_library());
        self.push_log(kind, text);
    }

    /// Prices follow farming: they're looked up while it runs, the game
    /// playing first, and a game's again when one of its cards drops.
    fn keep_prices_coming(&mut self) {
        if !self.farming.is_running() {
            return;
        }
        self.market.start();
        let Some(s) = &self.status else {
            return;
        };
        let drops = &s.session.drops;
        if drops.len() < self.seen_drops {
            self.seen_drops = 0;
        }
        let mut dropped: Vec<AppId> = drops[self.seen_drops..].iter().map(|d| d.app_id).collect();
        dropped.dedup();
        self.seen_drops = drops.len();
        let wanted = prices_wanted(&s.playing, &s.session, &s.order);
        for app_id in dropped {
            self.market.dropped(app_id);
        }
        self.market.want(wanted);
    }

    fn on_login_update(&mut self, u: SignInUpdate) {
        let Some(Overlay::SignIn(mut v)) = self.overlay.take() else {
            return;
        };
        if let Some(c) = u.challenge {
            v.challenge = Some(c);
        }
        if u.done {
            match u.err {
                None => {
                    self.account.refresh();
                    let name = self.account.get().map(|a| a.name).unwrap_or_default();
                    let who = if name.is_empty() {
                        String::new()
                    } else {
                        format!(" as {name}")
                    };
                    self.push_log(EventKind::Info, format!("Signed in to Steam{who}"));
                    // Another account's farming is a session of its own.
                    self.farming.signed_in();
                    self.library.invalidate();
                    self.status = None;
                    self.seen_drops = 0;
                    if self.onboarding.is_active() {
                        // Farming waits for the end of onboarding; next up is
                        // moving on, whatever was said about needing an account.
                        self.setup.cursor = CONTINUE_ROW;
                        self.flash = None;
                    } else if !self.paused_by_user {
                        // A new sign-in starts a new farmer.
                        self.farming.pause();
                        self.farming.start();
                    }
                    v.outcome = Some(Ok(name));
                }
                Some(e) => {
                    self.push_log(EventKind::Error, format!("Couldn't sign in: {e}"));
                    v.outcome = Some(Err(e));
                }
            }
            v.finished = Some(Instant::now());
        }
        self.overlay = Some(Overlay::SignIn(v));
    }

    fn push_log(&mut self, kind: EventKind, text: String) {
        self.log.push(LogEntry {
            at: (self.clock.now)().with_timezone(&(self.clock.zone)()),
            received: Instant::now(),
            kind,
            text,
        });
        if self.log.len() > MAX_LOG {
            self.log.drain(..self.log.len() - MAX_LOG);
        }
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
    }

    /// The library as it's best known: the farmer's, whose hours and drops
    /// are the latest, or the one read for browsing.
    pub(crate) fn known_library(&self) -> SteamLibrary {
        match &self.status {
            Some(s) if !s.library.is_empty() => s.library.clone(),
            _ => self.library.library().cloned().unwrap_or_default(),
        }
    }

    fn queue(&self) -> Queue {
        let prefs = self.farming.preferences();
        let (order, playing, mode) = match (&self.status, self.farming.is_running()) {
            (Some(s), true) => (s.order.clone(), s.playing.clone(), s.mode),
            (Some(s), false) => (s.order.clone(), Vec::new(), None),
            (None, _) => (Vec::new(), Vec::new(), None),
        };
        Queue::build(&self.known_library(), &order, &playing, mode, &prefs)
    }

    /// Games the cursor can land on, top to bottom.
    fn visible_ids(&self, q: &Queue) -> Vec<AppId> {
        q.entries()
            .filter(|e| self.show_done || e.section() != Section::Done)
            .map(|e| e.game.app_id)
            .collect()
    }

    fn sync_selection(&mut self) {
        let ids = self.visible_ids(&self.queue());
        if !self.selected.is_some_and(|s| ids.contains(&s)) {
            self.selected = ids.into_iter().next();
        }
    }

    fn move_selection(&mut self, delta: isize) {
        let ids = self.visible_ids(&self.queue());
        if ids.is_empty() {
            return;
        }
        let cur = self
            .selected
            .and_then(|s| ids.iter().position(|&i| i == s))
            .unwrap_or(0);
        let next = (cur as isize)
            .saturating_add(delta)
            .clamp(0, ids.len() as isize - 1);
        self.selected = Some(ids[next as usize]);
    }

    fn selected_entry(&self) -> Option<QueueEntry> {
        self.queue().get(self.selected?).cloned()
    }

    /// The onboarding's games, in the library's order.
    pub(crate) fn picks(&self) -> Vec<GameRow> {
        self.games.picks(&self.library.with_drops_left())
    }

    // ── Actions ──────────────────────────────────────────────────────────────

    fn set_tier(&mut self, tier: Tier) {
        let Some(e) = self.selected_entry() else {
            return;
        };
        let name = e.game.name.clone();
        if !e.game.has_drops_left() {
            self.flash(format!(
                "Every card has dropped for {name}: nothing left to farm."
            ));
            return;
        }
        if let Err(err) = self.farming.set_tier(e.game.app_id, tier) {
            return self.didnt_stick(err);
        }
        self.flash(match tier {
            Tier::Priority(n) => format!("{name} is now priority #{n}."),
            Tier::Indifferent => format!("{name} is back to indifferent."),
            Tier::Skip => format!("Skipped {name}: it won't be farmed."),
        });
    }

    /// Says a change didn't happen, rather than confirming one that didn't.
    fn didnt_stick(&mut self, e: PreferencesError) {
        self.flash(format!("That didn't stick — {e}. Try again?"));
    }

    fn open_card_page(&mut self) {
        let Some(e) = self.selected_entry() else {
            return;
        };
        let url = card_page(e.game.app_id);
        match open_in_browser(&url) {
            Ok(()) => self.flash(format!("Opened {}'s card page.", e.game.name)),
            Err(err) => self.flash(format!("Couldn't open a browser ({err}). Visit {url}")),
        }
    }

    fn toggle_pause(&mut self) {
        if self.farming.is_running() {
            self.farming.pause();
            self.paused_by_user = true;
            self.flash("Paused: nothing is played until you carry on. Press p.");
        } else if self.account.is_signed_in() {
            self.farming.start();
            self.paused_by_user = false;
            self.flash("Farming started.");
        } else {
            self.flash("Sign in first — press a.");
        }
    }

    /// Picks a game as a priority game, or unpicks it.
    fn toggle_game(&mut self, app_id: AppId, name: &str) {
        match self.games.toggle(app_id) {
            Ok(Some(n)) => self.flash(format!("{name} is priority #{n}: farmed first.")),
            Ok(None) => self.flash(format!("{name} is no longer a priority game.")),
            Err(e) => self.didnt_stick(e),
        }
    }

    fn toggle_only_priority(&mut self) {
        match self.games.toggle_only_priority() {
            Ok(()) if self.games.only_priority() => {
                self.flash("\"Only priority\" is on: only your priority games are farmed.")
            }
            Ok(()) => self.flash("\"Only priority\" is off: every game with cards left is farmed."),
            Err(e) => self.didnt_stick(e),
        }
    }

    fn toggle_appear_online(&mut self) {
        match self.games.toggle_appear_online() {
            Ok(()) if self.games.appear_online() => {
                self.flash("Showing as online while farming: friends see the games being played.")
            }
            Ok(()) => self.flash("Appearing offline while farming. Cards drop just the same."),
            Err(e) => self.didnt_stick(e),
        }
    }

    fn open_games(&mut self) -> Overlay {
        self.library.refresh_if_older(LIBRARY_FRESH);
        Overlay::Games(GamesView { cursor: 0 })
    }

    /// Moves onboarding on a step; after the last, farming starts.
    fn onboarding_next(&mut self) {
        if let Err(NeedsAccount) = self.onboarding.forward() {
            self.flash("Sign in with the Steam app first: farming needs your account.");
            return;
        }
        self.enter_step();
        if !self.onboarding.is_active() {
            self.paused_by_user = false;
            self.farming.start();
            self.flash("Farming started. Press g to choose games, ? for help.");
        }
    }

    fn onboarding_back(&mut self) {
        self.onboarding.back();
        self.enter_step();
    }

    /// A fresh cursor for the step now on screen.
    fn enter_step(&mut self) {
        self.setup = SetupView::default();
        match self.onboarding.step() {
            Some(Step::SignIn) if self.onboarding.can_continue() => {
                self.setup.cursor = CONTINUE_ROW;
            }
            Some(Step::Games) => self.library.refresh_if_older(LIBRARY_FRESH),
            _ => {}
        }
    }

    fn begin_login(&mut self, from_account: bool) -> Overlay {
        self.login.start();
        Overlay::SignIn(SignInView {
            challenge: None,
            outcome: None,
            finished: None,
            from_account,
        })
    }

    /// Forgets the sign-in and stops farming, ending its session: it's back
    /// to onboarding's sign-in step, as there's nothing to farm with.
    fn sign_out(&mut self) {
        match self.account.sign_out() {
            Ok(()) => {
                self.farming.pause();
                self.farming.end_session();
                self.market.stop();
                self.paused_by_user = false;
                self.status = None;
                self.seen_drops = 0;
                self.library.invalidate();
                self.onboarding.signed_out();
                self.enter_step();
                self.flash("Signed out. Sign in with the Steam app to carry on farming.");
            }
            Err(e) => self.flash(format!("Couldn't sign out — {e}.")),
        }
    }

    /// Where to go once a sign-in pop-up closes.
    fn after_login(&mut self, from_account: bool) -> Option<Overlay> {
        self.login.cancel();
        from_account.then_some(Overlay::Account { confirm: false })
    }

    // ── Keys ─────────────────────────────────────────────────────────────────

    fn on_event(&mut self, ev: Event) {
        let Event::Key(k) = ev else {
            return;
        };
        if k.kind == KeyEventKind::Release {
            return;
        }
        if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        self.key(k);
    }

    /// A key for whatever is on screen: the open pop-up, else onboarding,
    /// else the dashboard.
    fn key(&mut self, k: KeyEvent) {
        match (self.overlay.take(), self.onboarding.step()) {
            (Some(o), _) => self.overlay = self.overlay_key(o, k),
            (None, Some(step)) => self.onboarding_key(step, k),
            (None, None) => self.dashboard_key(k),
        }
        self.sync_selection();
    }

    fn onboarding_key(&mut self, step: Step, k: KeyEvent) {
        match (step, k.code) {
            (_, KeyCode::Char('q')) => self.quit = true,
            (_, KeyCode::Char('?')) => self.overlay = Some(Overlay::Help),
            (_, KeyCode::Right | KeyCode::Tab) => self.onboarding_next(),
            (_, KeyCode::Left | KeyCode::BackTab | KeyCode::Esc) => self.onboarding_back(),
            // Enter goes on, except where it acts on the row under the cursor.
            (Step::Welcome | Step::Games | Step::Start, KeyCode::Enter) => self.onboarding_next(),
            (Step::SignIn, KeyCode::Enter | KeyCode::Char(' ')) => {
                if self.setup.cursor < CONTINUE_ROW {
                    self.overlay = Some(self.begin_login(false));
                } else {
                    self.onboarding_next();
                }
            }
            (Step::SignIn, code) => {
                self.setup.cursor = moved(self.setup.cursor, code, CONTINUE_ROW + 1);
            }
            (Step::Games, KeyCode::Char(' ')) => {
                let rows = self.picks();
                if let Some(r) = rows.get(self.setup.cursor.min(rows.len().saturating_sub(1))) {
                    self.toggle_game(r.app_id, &r.name.clone());
                }
            }
            (Step::Games, KeyCode::Char('o')) => self.toggle_only_priority(),
            (Step::Games, KeyCode::Char('r')) => self.library.refresh(),
            (Step::Games, code) => {
                self.setup.cursor = moved(self.setup.cursor, code, self.picks().len());
            }
            (Step::Start, KeyCode::Char('v')) => self.toggle_appear_online(),
            _ => {}
        }
    }

    fn dashboard_key(&mut self, k: KeyEvent) {
        match k.code {
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-10),
            KeyCode::PageDown => self.move_selection(10),
            KeyCode::Home => self.move_selection(isize::MIN),
            KeyCode::End => self.move_selection(isize::MAX),
            KeyCode::Char(c @ '1'..='9') => {
                self.set_tier(Tier::Priority(c as usize - '0' as usize))
            }
            KeyCode::Char('0') | KeyCode::Backspace | KeyCode::Delete => {
                self.set_tier(Tier::Indifferent)
            }
            KeyCode::Char('x') => self.set_tier(Tier::Skip),
            KeyCode::Char('o') => self.open_card_page(),
            KeyCode::Char('c') => self.show_done = !self.show_done,
            KeyCode::Enter if self.selected.is_some() => {
                self.overlay = Some(Overlay::Detail { scroll: 0 });
            }
            KeyCode::Char('a') => self.overlay = Some(Overlay::Account { confirm: false }),
            KeyCode::Char('g') => self.overlay = Some(self.open_games()),
            KeyCode::Char('l') => {
                self.overlay = Some(Overlay::Log(LogView {
                    offset: usize::MAX,
                    max: 0,
                    follow: true,
                }));
            }
            KeyCode::Char('p') => self.toggle_pause(),
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Char('q') => {
                if self.farming.is_running() {
                    self.overlay = Some(Overlay::ConfirmQuit);
                } else {
                    self.quit = true;
                }
            }
            _ => {}
        }
    }

    /// Handles a key for the open pop-up; returns what should be open next.
    fn overlay_key(&mut self, o: Overlay, k: KeyEvent) -> Option<Overlay> {
        match o {
            Overlay::Help => match k.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?' | 'q') => None,
                _ => Some(Overlay::Help),
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

            Overlay::SignIn(v) => self.login_key(v, k),

            Overlay::Games(v) => self.games_key(v, k),

            Overlay::Log(mut v) => {
                let page = 10;
                match k.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('l' | 'q') => return None,
                    KeyCode::Up => {
                        v.offset = v.offset.min(v.max).saturating_sub(1);
                        v.follow = false;
                    }
                    KeyCode::PageUp => {
                        v.offset = v.offset.min(v.max).saturating_sub(page);
                        v.follow = false;
                    }
                    KeyCode::Down | KeyCode::PageDown => {
                        let step = if k.code == KeyCode::Down { 1 } else { page };
                        v.offset = v.offset.min(v.max) + step;
                        v.follow = v.offset >= v.max;
                    }
                    KeyCode::Home => {
                        v.offset = 0;
                        v.follow = false;
                    }
                    KeyCode::End => v.follow = true,
                    _ => {}
                }
                Some(Overlay::Log(v))
            }

            Overlay::Detail { mut scroll } => {
                match k.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return None,
                    KeyCode::Up => {
                        self.move_selection(-1);
                        scroll = 0;
                    }
                    KeyCode::Down => {
                        self.move_selection(1);
                        scroll = 0;
                    }
                    KeyCode::PageDown => scroll += 10,
                    KeyCode::PageUp => scroll = scroll.saturating_sub(10),
                    KeyCode::Char(c @ '1'..='9') => {
                        self.set_tier(Tier::Priority(c as usize - '0' as usize))
                    }
                    KeyCode::Char('0') | KeyCode::Backspace | KeyCode::Delete => {
                        self.set_tier(Tier::Indifferent)
                    }
                    KeyCode::Char('x') => self.set_tier(Tier::Skip),
                    KeyCode::Char('o') => self.open_card_page(),
                    _ => {}
                }
                Some(Overlay::Detail { scroll })
            }
        }
    }

    fn login_key(&mut self, v: SignInView, k: KeyEvent) -> Option<Overlay> {
        match (&v.outcome, k.code) {
            (Some(Err(_)), KeyCode::Char('r')) => Some(self.begin_login(v.from_account)),
            (Some(_), KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) => {
                self.after_login(v.from_account)
            }
            (None, KeyCode::Esc) => {
                self.flash("Stopped signing in.");
                self.after_login(v.from_account)
            }
            _ => Some(Overlay::SignIn(v)),
        }
    }

    fn games_key(&mut self, mut v: GamesView, k: KeyEvent) -> Option<Overlay> {
        let rows = |app: &Self| app.games.rows(app.known_library().games());
        // Keeps the cursor on a game that moved, e.g. to its new rank.
        let follow = |app: &Self, app_id: AppId| rows(app).iter().position(|x| x.app_id == app_id);
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
                    match self
                        .games
                        .set_tier(r.app_id, Tier::Priority(c as usize - '0' as usize))
                    {
                        Ok(()) => v.cursor = follow(self, r.app_id).unwrap_or(0),
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
            code => v.cursor = moved(v.cursor, code, all.len()),
        }
        Some(Overlay::Games(v))
    }

    // ── Drawing ──────────────────────────────────────────────────────────────

    fn draw(&mut self, f: &mut Frame<'_>) {
        let area = f.area();
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            dashboard_view::too_small(f, area);
            return;
        }
        let prefs = self.farming.preferences();
        let queue = self.queue();
        let account = self.account.get();
        let library = self.known_library();
        let order = match &self.status {
            Some(s) => s.order.clone(),
            None => farm_order(&library, &prefs, &[]),
        };
        let no_session = Session::default();
        let no_sets = CardSets::default();
        let book = self.market.book();
        let wallet = self.market.wallet();
        let (queue_offset, log_scroll) = {
            let cx = Ctx {
                app: self,
                queue: &queue,
                account: account.as_ref(),
                prefs: &prefs,
                now: (self.clock.now)(),
                zone: (self.clock.zone)(),
                library: &library,
                order: &order,
                sets: self.status.as_ref().map_or(&no_sets, |s| &s.sets),
                session: self.status.as_ref().map_or(&no_session, |s| &s.session),
                book: &book,
                wallet: wallet.as_ref(),
            };
            let queue_offset = match self.onboarding.step() {
                Some(step) => {
                    onboarding_view::render(f, area, &cx, step);
                    self.queue_offset
                }
                None => dashboard_view::render(f, area, &cx),
            };
            let log_scroll = popups::render(f, area, &cx);
            (queue_offset, log_scroll)
        };
        self.queue_offset = queue_offset;
        match (log_scroll, self.overlay.as_mut()) {
            (Some((offset, max)), Some(Overlay::Log(v))) => {
                v.offset = offset;
                v.max = max;
            }
            (Some((offset, _)), Some(Overlay::Detail { scroll })) => *scroll = offset,
            _ => {}
        }
    }
}

/// Where a list cursor lands after a movement key, within `len` rows.
fn moved(cursor: usize, code: KeyCode, len: usize) -> usize {
    let last = len.saturating_sub(1);
    let cursor = cursor.min(last);
    match code {
        KeyCode::Up => cursor.saturating_sub(1),
        KeyCode::Down => (cursor + 1).min(last),
        KeyCode::PageUp => cursor.saturating_sub(10),
        KeyCode::PageDown => (cursor + 10).min(last),
        KeyCode::Home => 0,
        KeyCode::End => last,
        _ => cursor,
    }
}
