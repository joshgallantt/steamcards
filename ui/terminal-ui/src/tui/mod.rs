// TUI — onboarding on first run, then a single dashboard that stays up the
// whole time, with pop-ups for the account, signing in, games, the log and
// help. Renders view-model state and forwards keys to view models; no
// business logic lives here.

mod dashboard;
mod onboarding;
mod overlays;
#[cfg(test)]
mod preview;
mod theme;
mod widgets;

use std::{
    collections::HashMap,
    io,
    time::{Duration, Instant},
};

use account::{Account as SignedIn, LoginChallenge};
use chrono::{DateTime, FixedOffset, Local, Utc};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use farming::{EventKind, FarmingEvent, FarmingStatus};
use futures::StreamExt;
use library::SteamLibrary;
use preferences::{Preferences, PreferencesError, Tier};
use ratatui::{DefaultTerminal, Frame, Terminal, backend::CrosstermBackend};

use crate::viewmodel::{
    Account, Farming, GameRow, Games, Library, Login, LoginUpdate, NeedsAccount, Onboarding, Queue,
    QueueEntry, Section, Step, card_page, open_in_browser,
};

const MAX_LOG: usize = 1000;
/// Below this width the details panel folds into a pop-up (enter).
const WIDE: u16 = 100;
const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 16;
const FLASH_FOR: Duration = Duration::from_secs(4);
const LOGIN_DONE_FOR: Duration = Duration::from_millis(1500);
/// How old the library may be when a screen showing it opens.
const LIBRARY_FRESH: Duration = Duration::from_secs(10 * 60);
/// The sign-in step lists the account, then "Continue".
const CONTINUE_ROW: usize = 1;

/// What time it is, and the time zone times are shown in: the system's when
/// steamcards runs, and fixed ones in the previews, so a screen looks the
/// same on any machine, on any day.
#[derive(Clone, Copy)]
struct Clock {
    now: fn() -> DateTime<Utc>,
    zone: fn() -> FixedOffset,
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

struct LogEntry {
    /// When it happened, in the time zone it's shown in.
    at: DateTime<FixedOffset>,
    received: Instant,
    kind: EventKind,
    text: String,
}

enum Overlay {
    Help,
    Account {
        /// Asking whether to sign out.
        confirm: bool,
    },
    Login(LoginView),
    Games(GamesView),
    Log(LogView),
    /// A game's details on windows too narrow for the side panel.
    Detail,
    ConfirmQuit,
}

struct LoginView {
    challenge: Option<LoginChallenge>,
    /// `Ok(account name)` or `Err(reason)` once the sign-in ends.
    outcome: Option<Result<String, String>>,
    finished: Option<Instant>,
    /// Return to the account pop-up (rather than the dashboard) afterwards.
    from_account: bool,
}

struct GamesView {
    cursor: usize,
}

/// Where the user is within an onboarding step.
#[derive(Default)]
struct SetupView {
    cursor: usize,
}

struct LogView {
    offset: usize,
    /// Largest offset at the last draw (the view's bottom).
    max: usize,
    /// Stick to the newest entries as they arrive.
    follow: bool,
}

/// Everything a frame needs, computed once per draw.
struct Ctx<'a> {
    app: &'a App,
    queue: &'a Queue,
    account: Option<&'a SignedIn>,
    prefs: &'a Preferences,
    now: DateTime<Utc>,
}

impl Ctx<'_> {
    fn selected(&self) -> Option<&QueueEntry> {
        self.app.selected.and_then(|id| self.queue.get(id))
    }

    fn spinner(&self) -> &'static str {
        theme::SPINNER[self.app.tick % theme::SPINNER.len()]
    }

    fn signed_in(&self) -> bool {
        self.account.is_some()
    }

    fn expired(&self) -> bool {
        self.account.is_some_and(|a| a.expired)
    }
}

pub struct App {
    account: Account,
    login: Login,
    farming: Farming,
    games: Games,
    library: Library,
    onboarding: Onboarding,
    setup: SetupView,

    /// The farmer's last word on what it's doing.
    status: Option<FarmingStatus>,
    log: Vec<LogEntry>,
    clock: Clock,
    /// Card drops received per game when first seen this session, to count
    /// the drops since.
    baseline: HashMap<u32, u32>,

    /// Game under the cursor, by app ID, so it stays put when the queue
    /// re-sorts.
    selected: Option<u32>,
    queue_offset: usize,
    show_done: bool,
    paused_by_user: bool,

    overlay: Option<Overlay>,
    /// Short-lived confirmation shown above the footer.
    flash: Option<(String, Instant)>,
    tick: usize,
    width: u16,
    quit: bool,
}

impl App {
    pub fn new(
        account: Account,
        login: Login,
        farming: Farming,
        games: Games,
        library: Library,
        onboarding: Onboarding,
    ) -> Self {
        Self {
            account,
            login,
            farming,
            games,
            library,
            onboarding,
            setup: SetupView::default(),
            status: None,
            log: Vec::new(),
            clock: Clock::SYSTEM,
            baseline: HashMap::new(),
            selected: None,
            queue_offset: 0,
            show_done: false,
            paused_by_user: false,
            overlay: None,
            flash: None,
            tick: 0,
            width: 0,
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
        while let Some(ev) = self.farming.try_recv() {
            self.on_farming_event(ev);
        }
        self.library.poll();
        if let Some(Overlay::Login(v)) = &self.overlay {
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

    fn on_farming_event(&mut self, ev: FarmingEvent) {
        if let Some(s) = ev.status {
            for g in s.library.games() {
                self.baseline.entry(g.app_id).or_insert(g.drops.received);
            }
            self.status = Some(s);
        }
        if !ev.message.is_empty() {
            self.push_log(ev.kind, ev.message);
        }
    }

    /// Cards dropped since steamcards started.
    fn dropped(&self) -> u32 {
        let Some(s) = &self.status else {
            return 0;
        };
        s.library
            .games()
            .iter()
            .map(|g| {
                let before = self
                    .baseline
                    .get(&g.app_id)
                    .copied()
                    .unwrap_or(g.drops.received);
                g.drops.received.saturating_sub(before)
            })
            .sum()
    }

    fn on_login_update(&mut self, u: LoginUpdate) {
        let Some(Overlay::Login(mut v)) = self.overlay.take() else {
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
                    self.library.invalidate();
                    self.status = None;
                    self.baseline.clear();
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
        self.overlay = Some(Overlay::Login(v));
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
    fn known_library(&self) -> SteamLibrary {
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
    fn visible_ids(&self, q: &Queue) -> Vec<u32> {
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
    fn picks(&self) -> Vec<GameRow> {
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
    fn toggle_game(&mut self, app_id: u32, name: &str) {
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
        Overlay::Login(LoginView {
            challenge: None,
            outcome: None,
            finished: None,
            from_account,
        })
    }

    /// Forgets the sign-in and stops farming: it's back to onboarding's
    /// sign-in step, as there's nothing to farm with.
    fn sign_out(&mut self) {
        match self.account.sign_out() {
            Ok(()) => {
                self.farming.pause();
                self.paused_by_user = false;
                self.status = None;
                self.baseline.clear();
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
            KeyCode::Enter if self.width < WIDE && self.selected.is_some() => {
                self.overlay = Some(Overlay::Detail);
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

            Overlay::Login(v) => self.login_key(v, k),

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

            Overlay::Detail => {
                match k.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return None,
                    KeyCode::Up => self.move_selection(-1),
                    KeyCode::Down => self.move_selection(1),
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
                Some(Overlay::Detail)
            }
        }
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
        self.width = area.width;
        if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
            dashboard::too_small(f, area);
            return;
        }
        let prefs = self.farming.preferences();
        let queue = self.queue();
        let account = self.account.get();
        let (queue_offset, log_scroll) = {
            let cx = Ctx {
                app: self,
                queue: &queue,
                account: account.as_ref(),
                prefs: &prefs,
                now: (self.clock.now)(),
            };
            let queue_offset = match self.onboarding.step() {
                Some(step) => {
                    onboarding::render(f, area, &cx, step);
                    self.queue_offset
                }
                None => dashboard::render(f, area, &cx),
            };
            let log_scroll = overlays::render(f, area, &cx);
            (queue_offset, log_scroll)
        };
        self.queue_offset = queue_offset;
        if let (Some((offset, max)), Some(Overlay::Log(v))) = (log_scroll, self.overlay.as_mut()) {
            v.offset = offset;
            v.max = max;
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
