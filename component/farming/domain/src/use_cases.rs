//! Farming cards: reading the library, playing what the plan says, looking at
//! the cards as they drop, and stepping aside while another device plays.

use std::{collections::HashMap, sync::Arc, time::Duration};

use chrono::Utc;
use library::{Game, LookAtGame, ReadLibrary, SteamLibrary};
use preferences::{GetPreferences, Preferences};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    EventKind, FarmingEvent, FarmingStatus, Mode, PlayRepository, Signal, Status,
    ranking::{Plan, can_drop, farm_order, plan, why_nothing},
    reporter::Reporter,
    rules::{
        AFTER_BLOCK, AFTER_NEW_ITEMS, GIVE_UP_AFTER, GIVE_UP_TIMES, HOURS_BEFORE_DROPS, IDLE_LOOK,
        LOOK_EVERY, LOOK_EVERY_LAST, RETRY_CONNECT, RETRY_READ, TICK,
    },
};

/// Farms until the token is cancelled, reporting on the channel. Runs
/// detached; the handle resolves once it has stopped, and stopped playing.
pub type FarmCards =
    Arc<dyn Fn(CancellationToken, mpsc::Sender<FarmingEvent>) -> JoinHandle<()> + Send + Sync>;

pub fn farm_cards(
    read: ReadLibrary,
    look: LookAtGame,
    play: Arc<dyn PlayRepository>,
    prefs: GetPreferences,
) -> FarmCards {
    let farmer = Arc::new(Farmer {
        read,
        look,
        play,
        prefs,
    });
    Arc::new(move |token, events| {
        let farmer = Arc::clone(&farmer);
        tokio::spawn(async move {
            let r = Reporter::new(events);
            farmer.run(&r, &token).await;
            farmer.play.stop().await;
        })
    })
}

struct Farmer {
    read: ReadLibrary,
    look: LookAtGame,
    play: Arc<dyn PlayRepository>,
    prefs: GetPreferences,
}

/// How a spell of playing ended.
enum Outcome {
    /// Done, or something changed: read the library and plan again.
    Again,
    /// Another device started playing.
    Blocked,
    /// The connection went.
    Lost(String),
    /// Another session took this one's place.
    Replaced,
    Stopped,
}

/// How waiting out another device ended.
enum Waited {
    Resume,
    Replaced,
    Stopped,
}

/// What woke the farmer while it played.
enum Woke {
    Look,
    Tick,
    Signal(Signal),
}

/// What one run of the farmer knows.
#[derive(Default)]
struct Run {
    library: SteamLibrary,
    /// Hours counted here while playing. The badge pages take a while to
    /// show them (about half an hour), so these are a floor under what the
    /// pages say.
    counted: HashMap<u32, f64>,
    /// Games set aside for a long while without a drop, and how often.
    set_aside: HashMap<u32, u8>,
}

impl Run {
    /// Takes in a fresh read of the library, keeping the hours counted here
    /// and the card sets already seen.
    fn take(&mut self, fresh: SteamLibrary) {
        let mut games = Vec::with_capacity(fresh.games().len());
        for game in fresh.games() {
            games.push(self.merged(game.clone()));
        }
        self.library = SteamLibrary::new(games);
    }

    /// Takes in a fresh look at one game.
    fn update(&mut self, fresh: Game) -> Game {
        let game = self.merged(fresh);
        self.library.update(game.clone());
        game
    }

    fn merged(&self, mut fresh: Game) -> Game {
        if let Some(&hours) = self.counted.get(&fresh.app_id) {
            fresh.hours = fresh.hours.max(hours);
        }
        if fresh.cards.is_empty()
            && let Some(known) = self.library.game(fresh.app_id)
        {
            fresh.cards = known.cards.clone();
        }
        fresh
    }

    /// Counts `played` towards each game's hours.
    fn count(&mut self, app_ids: &[u32], played: Duration) {
        for &app_id in app_ids {
            let Some(mut game) = self.library.game(app_id).cloned() else {
                continue;
            };
            game.hours += played.as_secs_f64() / 3600.0;
            self.counted.insert(app_id, game.hours);
            self.library.update(game);
        }
    }

    fn name(&self, app_id: u32) -> String {
        self.library
            .game(app_id)
            .map_or_else(|| format!("app {app_id}"), |g| g.name.clone())
    }
}

impl Farmer {
    async fn run(&self, r: &Reporter, token: &CancellationToken) {
        let mut run = Run::default();
        loop {
            if token.is_cancelled() {
                return;
            }
            if let Some(by) = self.play.blocked() {
                match self.wait_out(by, &run, r, token).await {
                    Waited::Resume => {}
                    Waited::Replaced => return replaced(r),
                    Waited::Stopped => return,
                }
            }
            r.stage(Status::Checking, "reading your badges…");
            match (self.read)().await {
                Ok(Ok(fresh)) => run.take(fresh),
                failed => {
                    let why = match failed {
                        Ok(Err(e)) => e.to_string(),
                        Err(e) => e.to_string(),
                        Ok(Ok(_)) => String::new(),
                    };
                    r.event(
                        EventKind::Error,
                        format!("Couldn't read your badges: {why}"),
                    );
                    r.stage(
                        Status::Error,
                        "couldn't read your badges — trying again in 5 minutes",
                    );
                    if !pause(RETRY_READ, token).await {
                        return;
                    }
                    continue;
                }
            }
            let prefs = (self.prefs)();
            let outcome = match plan(&run.library, &prefs, &run.set_aside) {
                Plan::Cards(app_id) => self.farm_cards(app_id, &mut run, r, token).await,
                Plan::Hours(app_ids) => self.build_hours(app_ids, &mut run, r, token).await,
                Plan::Nothing => self.rest(&run, r, token).await,
            };
            match outcome {
                Outcome::Again | Outcome::Blocked => {}
                Outcome::Stopped => return,
                Outcome::Replaced => return replaced(r),
                Outcome::Lost(why) => {
                    r.event(
                        EventKind::Warning,
                        format!("{why} — trying again in a minute"),
                    );
                    r.stage(Status::Error, &why);
                    if !pause(RETRY_CONNECT, token).await {
                        return;
                    }
                }
            }
        }
    }

    /// Plays one game on its own until its cards have dropped, looking at its
    /// card page every quarter of an hour, and as soon as Steam says new items
    /// arrived.
    async fn farm_cards(
        &self,
        app_id: u32,
        run: &mut Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let mut prefs = (self.prefs)();
        if let Err(e) = self.play.play(&[app_id], prefs.appear_online).await {
            return Outcome::Lost(e.to_string());
        }
        if self.play.blocked().is_some() {
            return Outcome::Blocked;
        }
        let Some(mut game) = run.library.game(app_id).cloned() else {
            return Outcome::Again;
        };
        r.event(
            EventKind::Playing,
            format!(
                "Farming {} — {} to drop",
                game.name,
                cards(game.drops.remaining)
            ),
        );
        let mut last_drop = Instant::now();
        let mut next_look = Instant::now() + look_every(&game);
        let mut counted_to = Instant::now();
        loop {
            self.report(run, &[app_id], Mode::Cards, next_look, &prefs, r);
            let woke = self.wait(Some(next_look), token).await;
            run.count(&[app_id], counted_to.elapsed());
            counted_to = Instant::now();
            match woke {
                None => return Outcome::Stopped,
                Some(Woke::Signal(signal)) => match signal {
                    Signal::Blocked(_) => return Outcome::Blocked,
                    Signal::Replaced => return Outcome::Replaced,
                    Signal::Lost(why) => return Outcome::Lost(why),
                    Signal::Unblocked => continue,
                    Signal::NewItems => tokio::time::sleep(AFTER_NEW_ITEMS).await,
                },
                Some(Woke::Tick) => {
                    let latest = (self.prefs)();
                    if latest == prefs {
                        continue;
                    }
                    if plan(&run.library, &latest, &run.set_aside) != Plan::Cards(app_id) {
                        r.event(EventKind::Switched, switching(&game.name, app_id, &latest));
                        return Outcome::Again;
                    }
                    if latest.appear_online != prefs.appear_online
                        && let Err(e) = self.play.play(&[app_id], latest.appear_online).await
                    {
                        return Outcome::Lost(e.to_string());
                    }
                    prefs = latest;
                    continue;
                }
                Some(Woke::Look) => {}
            }

            match (self.look)(app_id).await {
                Ok(Ok(fresh)) => {
                    let dropped = game.drops.remaining.saturating_sub(fresh.drops.remaining);
                    game = run.update(fresh);
                    if dropped > 0 {
                        last_drop = Instant::now();
                        run.set_aside.remove(&app_id);
                        r.event(EventKind::Dropped, dropped_message(&game, dropped));
                    } else {
                        r.event(
                            EventKind::Progress,
                            format!(
                                "{}: {} still to drop",
                                game.name,
                                cards(game.drops.remaining)
                            ),
                        );
                    }
                    if !game.has_drops_left() {
                        r.info(format!("Every card has dropped for {}.", game.name));
                        return Outcome::Again;
                    }
                }
                Ok(Err(e)) => r.event(
                    EventKind::Warning,
                    format!("Couldn't look at {}'s cards: {e}", game.name),
                ),
                Err(e) => r.event(
                    EventKind::Warning,
                    format!("Couldn't look at {}'s cards: {e}", game.name),
                ),
            }

            if last_drop.elapsed() >= GIVE_UP_AFTER {
                let times = run.set_aside.entry(app_id).or_default();
                *times += 1;
                let what_now = if *times >= GIVE_UP_TIMES {
                    "leaving it be: Steam may not drop its cards (a family-shared or \
                     free-to-play game, or one marked private)"
                } else {
                    "trying the others first"
                };
                r.event(
                    EventKind::Warning,
                    format!("No card from {} in 10 hours — {what_now}", game.name),
                );
                return Outcome::Again;
            }
            next_look = Instant::now() + look_every(&game);
        }
    }

    /// Plays games that don't have the hours for their cards to drop yet,
    /// together, until the first of them does. Steam's badge pages lag
    /// behind, so the hours are counted here.
    async fn build_hours(
        &self,
        app_ids: Vec<u32>,
        run: &mut Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let mut prefs = (self.prefs)();
        if let Err(e) = self.play.play(&app_ids, prefs.appear_online).await {
            return Outcome::Lost(e.to_string());
        }
        if self.play.blocked().is_some() {
            return Outcome::Blocked;
        }
        let lead = run.name(app_ids[0]);
        r.event(
            EventKind::Playing,
            if app_ids.len() == 1 {
                format!("Playing {lead} until it has 3 hours, when its cards can start dropping")
            } else {
                format!(
                    "Playing {} games together until {lead} has 3 hours, when its cards can \
                     start dropping",
                    app_ids.len()
                )
            },
        );
        let mut counted_to = Instant::now();
        loop {
            let ready_at = Instant::now() + until_ready(run, &app_ids);
            self.report(run, &app_ids, Mode::Hours, ready_at, &prefs, r);
            let woke = self.wait(Some(ready_at), token).await;
            run.count(&app_ids, counted_to.elapsed());
            counted_to = Instant::now();
            match woke {
                None => return Outcome::Stopped,
                Some(Woke::Signal(signal)) => match signal {
                    Signal::Blocked(_) => return Outcome::Blocked,
                    Signal::Replaced => return Outcome::Replaced,
                    Signal::Lost(why) => return Outcome::Lost(why),
                    Signal::Unblocked => continue,
                    // A card may have dropped for one of them: look again.
                    Signal::NewItems => {
                        tokio::time::sleep(AFTER_NEW_ITEMS).await;
                        return Outcome::Again;
                    }
                },
                Some(Woke::Look | Woke::Tick) => {}
            }
            if let Some(ready) = app_ids
                .iter()
                .find_map(|&id| run.library.game(id).filter(|g| can_drop(g)))
            {
                r.info(format!(
                    "{} has 3 hours now: its cards can drop.",
                    ready.name
                ));
                return Outcome::Again;
            }
            let latest = (self.prefs)();
            if latest != prefs {
                if plan(&run.library, &latest, &run.set_aside) != Plan::Hours(app_ids.clone()) {
                    r.event(
                        EventKind::Switched,
                        "Your choices changed: farming in the new order".into(),
                    );
                    return Outcome::Again;
                }
                if latest.appear_online != prefs.appear_online
                    && let Err(e) = self.play.play(&app_ids, latest.appear_online).await
                {
                    return Outcome::Lost(e.to_string());
                }
                prefs = latest;
            }
        }
    }

    /// Nothing to farm: signs off, and looks again in a few hours, or as
    /// soon as the user's choices change.
    async fn rest(&self, run: &Run, r: &Reporter, token: &CancellationToken) -> Outcome {
        self.play.stop().await;
        let prefs = (self.prefs)();
        let until = Instant::now() + IDLE_LOOK;
        r.status(FarmingStatus {
            status: Status::Idle,
            library: run.library.clone(),
            order: Vec::new(),
            next_look: Some(when(until)),
            note: why_nothing(&run.library, &prefs).into(),
            ..Default::default()
        });
        loop {
            tokio::select! {
                _ = token.cancelled() => return Outcome::Stopped,
                _ = tokio::time::sleep_until(until) => return Outcome::Again,
                _ = tokio::time::sleep(TICK) => {
                    if (self.prefs)() != prefs {
                        return Outcome::Again;
                    }
                }
            }
        }
    }

    /// Waits while another device plays, then a minute more before playing
    /// again, so as not to take over from it.
    async fn wait_out(
        &self,
        mut by: Option<u32>,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Waited {
        loop {
            r.status(FarmingStatus {
                status: Status::Blocked,
                library: run.library.clone(),
                order: farm_order(&run.library, &(self.prefs)(), &run.set_aside),
                blocked_by: by,
                note: "playing on another device — farming waits until it stops".into(),
                ..Default::default()
            });
            loop {
                let signal = tokio::select! {
                    _ = token.cancelled() => return Waited::Stopped,
                    signal = self.play.next_signal() => signal,
                };
                match signal {
                    Signal::Unblocked | Signal::Lost(_) => break,
                    Signal::Replaced => return Waited::Replaced,
                    Signal::Blocked(app) => {
                        by = app;
                        break;
                    }
                    Signal::NewItems => {}
                }
            }
            if self.play.blocked().is_some() {
                continue;
            }
            r.info("Playing elsewhere stopped: farming carries on in a minute.".into());
            r.stage(Status::Blocked, "carrying on in a minute…");
            tokio::select! {
                _ = token.cancelled() => return Waited::Stopped,
                _ = tokio::time::sleep(AFTER_BLOCK) => return Waited::Resume,
                signal = self.play.next_signal() => match signal {
                    Signal::Replaced => return Waited::Replaced,
                    Signal::Blocked(app) => by = app,
                    _ => return Waited::Resume,
                },
            }
        }
    }

    /// Waits until `until`, the next tick, or Steam says something; `None`
    /// when stopped.
    async fn wait(&self, until: Option<Instant>, token: &CancellationToken) -> Option<Woke> {
        let tick = Instant::now() + TICK;
        let wake = until.map_or(tick, |u| u.min(tick));
        tokio::select! {
            _ = token.cancelled() => None,
            _ = tokio::time::sleep_until(wake) => Some(if until.is_some_and(|u| Instant::now() >= u) {
                Woke::Look
            } else {
                Woke::Tick
            }),
            signal = self.play.next_signal() => Some(Woke::Signal(signal)),
        }
    }

    fn report(
        &self,
        run: &Run,
        playing: &[u32],
        mode: Mode,
        next_look: Instant,
        prefs: &Preferences,
        r: &Reporter,
    ) {
        r.status(FarmingStatus {
            status: Status::Farming,
            library: run.library.clone(),
            order: farm_order(&run.library, prefs, &run.set_aside),
            playing: playing.to_vec(),
            mode: Some(mode),
            blocked_by: None,
            next_look: Some(when(next_look)),
            note: String::new(),
        });
    }
}

fn replaced(r: &Reporter) {
    r.event(
        EventKind::Error,
        "Another session signed in with this account's login in steamcards' place, so \
         farming stopped rather than knock it off. Press p to start again."
            .into(),
    );
    r.stage(Status::Error, "stopped: another session took over");
}

/// Sleeps for `d`; false when stopped meanwhile.
async fn pause(d: Duration, token: &CancellationToken) -> bool {
    tokio::select! {
        _ = token.cancelled() => false,
        _ = tokio::time::sleep(d) => true,
    }
}

fn look_every(game: &Game) -> Duration {
    if game.drops.remaining == 1 {
        LOOK_EVERY_LAST
    } else {
        LOOK_EVERY
    }
}

/// How long until the first of these games has the hours to drop cards.
fn until_ready(run: &Run, app_ids: &[u32]) -> Duration {
    let most = app_ids
        .iter()
        .filter_map(|&id| run.library.game(id))
        .map(|g| g.hours)
        .fold(0.0, f64::max);
    Duration::from_secs_f64(((HOURS_BEFORE_DROPS - most) * 3600.0).max(1.0))
}

/// A tokio instant as a time on the clock, for the screen.
fn when(at: Instant) -> chrono::DateTime<Utc> {
    let left = at.saturating_duration_since(Instant::now());
    Utc::now() + chrono::Duration::from_std(left).unwrap_or_default()
}

/// "1 card", "3 cards".
fn cards(n: u32) -> String {
    if n == 1 {
        "1 card".into()
    } else {
        format!("{n} cards")
    }
}

/// "A card dropped for Portal 2 — 2 to go", or that it was the last.
fn dropped_message(game: &Game, dropped: u32) -> String {
    let what = if dropped == 1 {
        "A card".to_owned()
    } else {
        format!("{dropped} cards")
    };
    match game.drops.remaining {
        0 => format!("{what} dropped for {} — that's all of them", game.name),
        left => format!("{what} dropped for {} — {left} to go", game.name),
    }
}

/// Why farming moved on from `name` after the user's choices changed.
fn switching(name: &str, app_id: u32, prefs: &Preferences) -> String {
    if prefs.wants(app_id) {
        format!("Moving on from {name}: something is ranked higher now")
    } else {
        format!("Moving on from {name}: it isn't to be farmed now")
    }
}
