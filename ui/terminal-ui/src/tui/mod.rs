// TUI — onboarding on first run, then a single dashboard that stays up the
// whole time, with pop-ups over it: a game's details, this session's cards,
// the market, games & settings, the account, signing in, the log, help and
// quitting. Renders view-model state and forwards keys to view models; no
// business logic lives here.

mod dashboard;
#[cfg(test)]
mod fixtures;
pub(crate) mod format;
#[cfg(test)]
mod golden;
mod layout;
mod onboarding;
mod popups;
#[cfg(test)]
mod preview;
mod small;
mod text;
mod theme;

use std::{
    io,
    sync::Arc,
    time::{Duration, Instant},
};

use account::Account as SignedIn;
use chrono::{DateTime, FixedOffset, Local, TimeDelta, Utc};
use crossterm::{
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use farming::{FarmingEvent, FarmingSession, FarmingStatus, Forecast, farm_order, forecast};
use futures::StreamExt;
use library::SteamLibrary;
use market::{Basis, MarketError, MarketEventKind, Money, PriceBook, Wallet};
use preferences::{Preferences, PreferencesError, Tier};
use ratatui::{DefaultTerminal, Frame, Terminal, backend::CrosstermBackend};

use crate::viewmodel::{
    Account, Activity, ChosenGame, Farming, Flash, GameRow, Games, Library, LogEntry, LogKind,
    Login, LoginUpdate, Market, NeedsAccount, Onboarding, Progress, Queue, QueueEntry, Run,
    Section, Snapshot, Step, Strip, Values, card_page, market_line, open_in_browser,
};
use layout::Regions;
use popups::{GamesView, HaulView, LogView, LoginView, MarketList, Overlay, Scroll};

const MAX_LOG: usize = 1000;
const FLASH_FOR: Duration = Duration::from_secs(4);
const LOGIN_DONE_FOR: Duration = Duration::from_millis(1500);
/// How old the library may be when a screen showing it opens.
const LIBRARY_FRESH: Duration = Duration::from_secs(10 * 60);
/// The sign-in step lists the account, then "Continue".
const CONTINUE_ROW: usize = 1;
/// The time to finish is worked out again once a minute, as well as when a
/// card drops or the order changes (docs/design/ui.md §5.7).
const FORECAST_EVERY: TimeDelta = TimeDelta::minutes(1);

/// What time it is, and the time zone times are shown in: the system's when
/// steamcards runs, and a fixed one in the previews and the screens' tests,
/// so a screen looks the same on any machine, on any day.
#[derive(Debug, Clone, Copy)]
enum Clock {
    System,
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "the clock of the previews and the screens' tests")
    )]
    Fixed {
        now: DateTime<Utc>,
        zone: FixedOffset,
    },
}

impl Clock {
    fn now(&self) -> DateTime<Utc> {
        match self {
            Self::System => Utc::now(),
            Self::Fixed { now, .. } => *now,
        }
    }

    /// The system's time zone, as it is now (it changes with daylight
    /// saving).
    fn zone(&self) -> FixedOffset {
        match self {
            Self::System => *Local::now().offset(),
            Self::Fixed { zone, .. } => *zone,
        }
    }
}

/// A line of the log, and when it came, for the strip's 4 seconds.
struct Logged {
    entry: LogEntry,
    received: Instant,
}

/// Where the user is within an onboarding step.
#[derive(Default)]
struct SetupView {
    cursor: usize,
}

/// The domain values a frame is built from, as they stand: owned, so a
/// `Snapshot` can borrow them.
struct Known {
    prefs: Preferences,
    account: Option<SignedIn>,
    library: SteamLibrary,
    prices: Arc<PriceBook>,
    wallet: Option<Wallet>,
    basis: Basis,
}

/// What a frame is drawn from: the App, the domain values it knows as they
/// stand, the queue and Progress the dashboard draws, where each region
/// goes, and the spinner's frame.
struct Ctx<'a> {
    app: &'a App,
    s: Snapshot<'a>,
    queue: &'a Queue,
    progress: &'a Progress,
    regions: Regions,
    spinner: &'static str,
}

pub struct App {
    account: Account,
    login: Login,
    farming: Farming,
    games: Games,
    library: Library,
    onboarding: Onboarding,
    market: Market,
    setup: SetupView,

    /// The farmer's last word on what it's doing.
    status: Option<FarmingStatus>,
    /// The time to finish, as last worked out, and what it was worked out
    /// for: the session's drops and the farm order.
    forecast: Option<Forecast>,
    forecast_for: (usize, Vec<u32>),
    /// What the session's first estimate said the cards would be worth on
    /// completion, kept as it's made for the end-of-library summary.
    estimated: Option<Money>,
    /// The line that told of Steam's pause on prices: the strip's alert
    /// while the pause lasts.
    paused_by: Option<LogEntry>,
    log: Vec<Logged>,
    clock: Clock,

    /// Game under the cursor, by app ID, so it stays put when the queue
    /// re-sorts.
    selected: Option<u32>,
    queue_offset: usize,
    show_done: bool,
    /// ≈ DONE IN as clock times rather than time left (t).
    clock_times: bool,
    paused_by_user: bool,

    overlay: Option<Overlay>,
    /// Short-lived confirmation shown on the strip.
    flash: Option<(String, Instant)>,
    /// The flash says a change didn't stick.
    flash_failed: bool,
    tick: usize,
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
        market: Market,
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
            forecast: None,
            forecast_for: (0, Vec::new()),
            estimated: None,
            paused_by: None,
            log: Vec::new(),
            clock: Clock::System,
            selected: None,
            queue_offset: 0,
            show_done: false,
            clock_times: false,
            paused_by_user: false,
            overlay: None,
            flash: None,
            flash_failed: false,
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
            self.start_farming();
        }
        let result = self.event_loop(&mut terminal).await;
        self.login.cancel();
        self.farming.stop().await;
        self.market.stop().await;

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
        while let Some(news) = self.market.try_recv() {
            let line = {
                let known = self.known();
                market_line(&news, &self.snapshot(&known))
            };
            match news.event.kind {
                MarketEventKind::Paused(_) => self.paused_by = Some(line.clone()),
                MarketEventKind::Resumed => self.paused_by = None,
                _ => {}
            }
            self.push_entry(line);
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
        self.update_forecast();
        self.ask_the_market();
        self.sync_selection();
    }

    fn on_farming_event(&mut self, ev: FarmingEvent) {
        if let Some(s) = ev.status {
            // A card dropped: its game's prices are looked at again, if
            // they're over an hour old.
            let before = self
                .status
                .as_ref()
                .map_or(0, |old| old.session.drops.len());
            let dropped: Vec<u32> = s
                .session
                .drops
                .iter()
                .skip(before)
                .map(|d| d.app_id)
                .collect();
            for app_id in dropped {
                self.market.refresh(app_id);
            }
            self.status = Some(s);
        }
        if !ev.message.is_empty() {
            self.push_log(LogKind::of(ev.kind), ev.message);
        }
    }

    /// Cards dropped this session.
    fn dropped(&self) -> u32 {
        self.status.as_ref().map_or(0, |s| {
            u32::try_from(s.session.drops.len()).unwrap_or(u32::MAX)
        })
    }

    /// Works the time to finish out again when a card has dropped, the farm
    /// order has changed, or a minute has passed, and only while farming
    /// runs: farming time is what it counts, so it holds still otherwise.
    fn update_forecast(&mut self) {
        let Some(status) = &self.status else {
            self.size_up();
            return;
        };
        if status.library.is_empty() {
            return;
        }
        let now = self.clock.now();
        let runs = {
            let known = self.known();
            Activity::of(&self.snapshot(&known)).runs()
        };
        let key = (status.session.drops.len(), status.order.clone());
        let due = self
            .forecast
            .as_ref()
            .is_none_or(|f| key != self.forecast_for || now - f.made_at >= FORECAST_EVERY);
        if !due || (!runs && self.forecast.is_some()) {
            return;
        }
        self.forecast = Some(forecast(
            &status.session,
            &status.library,
            &status.order,
            now,
        ));
        self.forecast_for = key;
        if self.estimated.is_none() && status.session.first_forecast.is_some() {
            let known = self.known();
            self.estimated = Values::build(&self.snapshot(&known)).map(|v| v.completion.value);
        }
    }

    /// Before farming starts, the Start step sizes up the job: the time to
    /// finish at 30 minutes a drop, the games taken in the order the farmer
    /// will take them. It's worked out again when that order changes.
    fn size_up(&mut self) {
        if !self.onboarding.is_active() {
            return;
        }
        let library = self.known_library();
        if library.is_empty() {
            return;
        }
        let order = farm_order(&library, &self.farming.preferences(), &[]);
        let key = (0, order);
        if self.forecast.is_some() && key == self.forecast_for {
            return;
        }
        self.forecast = Some(forecast(
            &FarmingSession::default(),
            &library,
            &key.1,
            self.clock.now(),
        ));
        self.forecast_for = key;
    }

    /// Tells the market what to price, most urgent first: the game being
    /// farmed, then the games with cards this session, the newest first,
    /// then the farm order. While values are on the instant basis, the
    /// cards held have their order books looked up.
    fn ask_the_market(&mut self) {
        let Some(status) = &self.status else {
            return;
        };
        let mut wanted: Vec<u32> = status.playing.clone();
        for d in status.session.drops.iter().rev() {
            if !wanted.contains(&d.app_id) {
                wanted.push(d.app_id);
            }
        }
        for &id in &status.order {
            if !wanted.contains(&id) {
                wanted.push(id);
            }
        }
        let held: Vec<String> = status
            .session
            .drops
            .iter()
            .filter_map(|d| match &d.card {
                farming::DropCard::Identified(asset) => Some(asset.market_hash_name.clone()),
                _ => None,
            })
            .collect();
        self.market.want(wanted);
        if self.market.basis() == Basis::Instant && !held.is_empty() {
            self.market.ask_offers(held, self.clock.now());
        }
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
                    self.push_log(LogKind::Info, format!("Signed in to Steam{who}"));
                    // Another account's farming is a session of its own.
                    self.farming.signed_in();
                    self.library.invalidate();
                    self.status = None;
                    self.forecast = None;
                    if self.onboarding.is_active() {
                        // Farming waits for the end of onboarding; next up is
                        // moving on, whatever was said about needing an account.
                        self.setup.cursor = CONTINUE_ROW;
                        self.flash = None;
                    } else if !self.paused_by_user {
                        // A new sign-in starts a new farmer.
                        self.farming.pause();
                        self.start_farming();
                    }
                    v.outcome = Some(Ok(name));
                }
                Some(e) => {
                    self.push_log(LogKind::Error, format!("Couldn't sign in: {e}"));
                    v.outcome = Some(Err(e));
                }
            }
            v.finished = Some(Instant::now());
        }
        self.overlay = Some(Overlay::Login(v));
    }

    fn push_log(&mut self, kind: LogKind, text: String) {
        let at = self.clock.now();
        self.push_entry(LogEntry { at, kind, text });
    }

    fn push_entry(&mut self, entry: LogEntry) {
        self.log.push(Logged {
            entry,
            received: Instant::now(),
        });
        if self.log.len() > MAX_LOG {
            self.log.drain(..self.log.len() - MAX_LOG);
        }
    }

    fn flash(&mut self, msg: impl Into<String>) {
        self.flash = Some((msg.into(), Instant::now()));
        self.flash_failed = false;
    }

    /// A flash that says a change didn't stick.
    fn flash_failure(&mut self, msg: impl Into<String>) {
        self.flash(msg);
        self.flash_failed = true;
    }

    /// What the strip shows: a flash for 4 seconds, a new event for as
    /// long, an alert while it lasts, else the newest event.
    fn strip(&self, s: &Snapshot<'_>) -> Strip {
        let flash = self
            .flash
            .as_ref()
            .filter(|(_, at)| at.elapsed() < FLASH_FOR)
            .map(|(text, _)| Flash {
                text: text.clone(),
                failed: self.flash_failed,
            });
        let newest = self
            .log
            .iter()
            .rev()
            .find(|l| l.entry.kind != LogKind::Progress)
            .map(|l| (&l.entry, l.received.elapsed() < FLASH_FOR));
        Strip::build(s, flash, newest, self.paused_by.as_ref())
    }

    /// The library as it's best known: the farmer's, whose hours and drops
    /// are the latest, or the one read for browsing.
    fn known_library(&self) -> SteamLibrary {
        match &self.status {
            Some(s) if !s.library.is_empty() => s.library.clone(),
            _ => self.library.library().cloned().unwrap_or_default(),
        }
    }

    /// The domain values this frame is built from.
    fn known(&self) -> Known {
        Known {
            prefs: self.farming.preferences(),
            account: self.account.get(),
            library: self.known_library(),
            prices: self.market.book(),
            wallet: self.market.wallet(),
            basis: self.market.basis(),
        }
    }

    /// What every view model is built from this frame.
    fn snapshot<'a>(&'a self, known: &'a Known) -> Snapshot<'a> {
        Snapshot {
            status: self.status.as_ref(),
            library: &known.library,
            run: if self.farming.is_running() {
                Run::Running
            } else if self.paused_by_user {
                Run::Paused
            } else {
                Run::Stopped
            },
            account: known.account.as_ref(),
            prefs: &known.prefs,
            forecast: self.forecast.as_ref(),
            prices: &known.prices,
            wallet: known.wallet,
            basis: known.basis,
            pause: self.market.pause(),
            now: self.clock.now(),
            zone: self.clock.zone(),
        }
    }

    fn queue(&self) -> Queue {
        let known = self.known();
        Queue::build(&self.snapshot(&known), self.selected)
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

    /// Starts farming, and pricing with it.
    fn start_farming(&mut self) {
        self.farming.start();
        if self.farming.is_running() {
            self.market.start();
        }
    }

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
            Tier::Priority(n) => format!("{name} is priority #{n}: farmed first."),
            Tier::Indifferent => format!("{name} is back to indifferent."),
            Tier::Skip => format!("Skipped {name}: it won't be farmed."),
        });
    }

    /// Says a change didn't happen, rather than confirming one that didn't.
    fn didnt_stick(&mut self, e: PreferencesError) {
        self.flash_failure(format!("That didn't stick: {e}."));
    }

    /// Values money on the next basis: list, then net, then instant.
    fn next_basis(&mut self) {
        match self.market.next_basis() {
            Ok(Basis::List) => self.flash("Values are now list prices: what buyers pay."),
            Ok(Basis::Net) => self.flash(
                "Values are now after fees: what you'd get listing each card at its lowest price.",
            ),
            Ok(Basis::Instant) => {
                self.flash("Values are now what selling at once gets: the best offer, after fees.")
            }
            Err(MarketError::Unavailable) => {
                self.flash_failure("That didn't stick: the market settings couldn't be saved.")
            }
            Err(e) => self.flash_failure(format!("That didn't stick: {e}.")),
        }
    }

    /// ≈ DONE IN as clock times, or as time left.
    fn toggle_clock_times(&mut self) {
        self.clock_times = !self.clock_times;
        self.flash(if self.clock_times {
            "≈ DONE IN shows clock times."
        } else {
            "≈ DONE IN shows the time left."
        });
    }

    /// The chosen game's details; its cards' order books are looked up for
    /// their sell-now prices.
    fn open_details(&mut self) -> Option<Overlay> {
        let app_id = self.selected?;
        let hashes = {
            let known = self.known();
            ChosenGame::market_hash_names(&self.snapshot(&known), app_id)
        };
        self.market.ask_offers(hashes, self.clock.now());
        Some(Overlay::Details(Scroll::default()))
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
            self.start_farming();
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
        Overlay::Games(GamesView::default())
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
            self.start_farming();
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

    /// Forgets the sign-in and stops farming, ending its session: it's back
    /// to onboarding's sign-in step, as there's nothing to farm with.
    fn sign_out(&mut self) {
        match self.account.sign_out() {
            Ok(()) => {
                self.farming.pause();
                self.farming.end_session();
                self.market.cancel();
                self.paused_by_user = false;
                self.status = None;
                self.forecast = None;
                self.estimated = None;
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
            (_, KeyCode::Char('?')) => self.overlay = Some(Overlay::Help(Scroll::default())),
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
            (Step::Start, KeyCode::Char('b')) => self.next_basis(),
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
            KeyCode::Enter => self.overlay = self.open_details(),
            KeyCode::Char('h') => self.overlay = Some(Overlay::Haul(HaulView::default())),
            KeyCode::Char('m') => self.overlay = Some(Overlay::Market(MarketList::default())),
            KeyCode::Char('b') => self.next_basis(),
            KeyCode::Char('t') => self.toggle_clock_times(),
            KeyCode::Char('a') => self.overlay = Some(Overlay::Account { confirm: false }),
            KeyCode::Char('g') => self.overlay = Some(self.open_games()),
            KeyCode::Char('l') => self.overlay = Some(Overlay::Log(LogView::default())),
            KeyCode::Char('p') => self.toggle_pause(),
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help(Scroll::default())),
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

    // ── Drawing ──────────────────────────────────────────────────────────────

    fn draw(&mut self, f: &mut Frame<'_>) {
        let area = f.area();
        let known = self.known();
        let spinner = theme::SPINNER[self.tick % theme::SPINNER.len()];
        let Some(regions) = layout::regions(area.width, area.height) else {
            small::render(f, &self.snapshot(&known), spinner);
            return;
        };
        // The pop-up is out of the App while it's drawn, which notes how far
        // it's scrolled.
        let mut overlay = self.overlay.take();
        let queue_offset = {
            let s = self.snapshot(&known);
            let queue = Queue::build(&s, self.selected);
            let progress = Progress::build(&s, self.estimated);
            // The session's summary takes more rows at M (mockup e), and a
            // pop-up covers the panels where they're drawn.
            let regions = if progress.summary.is_some() {
                regions.with_summary()
            } else {
                regions
            };
            let cx = Ctx {
                app: self,
                s,
                queue: &queue,
                progress: &progress,
                regions,
                spinner,
            };
            let buf = f.buffer_mut();
            let queue_offset = match self.onboarding.step() {
                Some(step) => {
                    onboarding::render(buf, &cx, step);
                    self.queue_offset
                }
                None => dashboard::render(buf, &cx),
            };
            if let Some(o) = overlay.as_mut() {
                popups::render(buf, &cx, o);
            }
            queue_offset
        };
        self.overlay = overlay;
        self.queue_offset = queue_offset;
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
