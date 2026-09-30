//! Farming cards: reading the library, playing what the plan says, looking at
//! the cards as they drop, telling which card each was, and stepping aside
//! while another device plays. What a session has seen outlasts a run of the
//! farmer: the `SessionKeeper` keeps it until `EndSession`.

use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use chrono::Utc;
use library::{CardAsset, DescribeCards, Game, LookAtGame, ReadLibrary, SteamLibrary};
use preferences::{GetPreferences, Preferences};
use tokio::{sync::mpsc, task::JoinHandle, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    Drop, DropCard, EventKind, FarmingEvent, FarmingStatus, Mode, PlayRepository, Signal, Status,
    ranking::{Plan, can_drop, farm_order, hours_to_go, plan, why_nothing},
    reporter::Reporter,
    rules::{
        AFTER_BLOCK, AFTER_NEW_ITEMS, GIVE_UP_AFTER, GIVE_UP_TIMES, HOURS_BEFORE_DROPS, IDLE_LOOK,
        LOOK_EVERY, LOOK_EVERY_LAST, RETRY_CONNECT, RETRY_READ, TICK,
    },
    session::{Found, Kept, SessionKeeper},
};

/// Farms until the token is cancelled, reporting on the channel. Runs
/// detached; the handle resolves once it has stopped, and stopped playing.
/// A run carries on the session the last one left, until it's ended.
pub type FarmCards =
    Arc<dyn Fn(CancellationToken, mpsc::Sender<FarmingEvent>) -> JoinHandle<()> + Send + Sync>;

/// Ends the farming session: the next run of [`FarmCards`] starts a new one,
/// with no drops, no hours counted and nothing set aside. Signing out ends
/// it.
pub type EndSession = Arc<dyn Fn() + Send + Sync>;

pub fn farm_cards(
    read: ReadLibrary,
    look: LookAtGame,
    describe: DescribeCards,
    play: Arc<dyn PlayRepository>,
    prefs: GetPreferences,
    sessions: Arc<SessionKeeper>,
) -> FarmCards {
    let farmer = Arc::new(Farmer {
        read,
        look,
        describe,
        play,
        prefs,
        sessions,
    });
    Arc::new(move |token, events| {
        let farmer = Arc::clone(&farmer);
        tokio::spawn(async move {
            let r = Reporter::new(events);
            let run = Run {
                kept: farmer.sessions.current(Utc::now()),
            };
            farmer.run(&run, &r, &token).await;
            farmer.play.stop().await;
        })
    })
}

pub fn end_session(sessions: Arc<SessionKeeper>) -> EndSession {
    Arc::new(move || sessions.end())
}

struct Farmer {
    read: ReadLibrary,
    look: LookAtGame,
    describe: DescribeCards,
    play: Arc<dyn PlayRepository>,
    prefs: GetPreferences,
    sessions: Arc<SessionKeeper>,
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

/// One run of the farmer, from starting to stopping, and the session it
/// carries on.
struct Run {
    kept: Arc<Mutex<Kept>>,
}

impl Run {
    fn kept(&self) -> MutexGuard<'_, Kept> {
        self.kept.lock().unwrap()
    }

    /// Tells the screens what the session holds now.
    fn tell(&self, r: &Reporter) {
        let (library, session, set_aside) = {
            let kept = self.kept();
            (
                kept.library.clone(),
                kept.session.clone(),
                kept.set_aside.clone(),
            )
        };
        r.amend(|s| {
            s.library = library;
            s.session = session;
            s.set_aside = set_aside;
        });
    }
}

impl Farmer {
    async fn run(&self, run: &Run, r: &Reporter, token: &CancellationToken) {
        loop {
            if token.is_cancelled() {
                return;
            }
            if let Some(by) = self.play.blocked() {
                match self.wait_out(by, run, r, token).await {
                    Waited::Resume => {}
                    Waited::Replaced => return replaced(r),
                    Waited::Stopped => return,
                }
            }
            r.stage(Status::Checking, "reading your badges…");
            match (self.read)().await {
                Ok(Ok(fresh)) => {
                    let found = run.kept().take(fresh, Utc::now());
                    self.dropped(found, &(self.prefs)(), run, r).await;
                }
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
            let planned = {
                let kept = run.kept();
                plan(&kept.library, &prefs, &kept.set_aside)
            };
            let outcome = match planned {
                Plan::Cards(app_id) => self.farm_cards(app_id, run, r, token).await,
                Plan::Hours(app_ids) => self.build_hours(app_ids, run, r, token).await,
                Plan::Nothing => self.rest(run, r, token).await,
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

    /// Plays one game on its own until its cards have dropped, as one
    /// stretch of farming alone.
    async fn farm_cards(
        &self,
        app_id: u32,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let prefs = (self.prefs)();
        if let Err(e) = self.play.play(&[app_id], prefs.appear_online).await {
            return Outcome::Lost(e.to_string());
        }
        if self.play.blocked().is_some() {
            return Outcome::Blocked;
        }
        let Some(game) = run.kept().library.game(app_id).cloned() else {
            return Outcome::Again;
        };
        let stretch = run.kept().start(&[app_id], Mode::Cards, Utc::now());
        let outcome = self.farm_alone(game, prefs, run, r, token).await;
        run.kept().stop(stretch, Utc::now());
        outcome
    }

    /// Looks at the card page of the game being farmed every quarter of an
    /// hour, and as soon as Steam says new items arrived.
    async fn farm_alone(
        &self,
        mut game: Game,
        mut prefs: Preferences,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let app_id = game.app_id;
        r.event(
            EventKind::Playing,
            format!(
                "Farming {} — {} to drop",
                game.name,
                cards(game.drops.remaining)
            ),
        );
        let mut last_drop = Instant::now();
        // A set not read yet is looked at first, as ASF looks at a game as it
        // starts on it: then a card that drops can be told by the set's
        // counts, and which copy it is.
        let mut next_look = if game.cards.is_empty() {
            Instant::now()
        } else {
            Instant::now() + look_every(&game)
        };
        let mut counted_to = Instant::now();
        loop {
            self.report(run, &[app_id], Mode::Cards, next_look, &prefs, r);
            let woke = self.wait(Some(next_look), token).await;
            run.kept().count(&[app_id], counted_to.elapsed());
            counted_to = Instant::now();
            match woke {
                None => return Outcome::Stopped,
                Some(Woke::Signal(signal)) => match signal {
                    Signal::Blocked(_) => return Outcome::Blocked,
                    Signal::Replaced => return Outcome::Replaced,
                    Signal::Lost(why) => return Outcome::Lost(why),
                    Signal::Unblocked => continue,
                    Signal::NewItems(items) => {
                        run.kept().keep_announced(items);
                        r.event(EventKind::Progress, "Steam says new items arrived".into());
                        tokio::time::sleep(AFTER_NEW_ITEMS).await;
                    }
                },
                Some(Woke::Tick) => {
                    let latest = (self.prefs)();
                    if latest == prefs {
                        continue;
                    }
                    let planned = {
                        let kept = run.kept();
                        plan(&kept.library, &latest, &kept.set_aside)
                    };
                    if planned != Plan::Cards(app_id) {
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
                    let (latest, found) = run.kept().update(fresh, Utc::now());
                    game = latest;
                    match found {
                        Some(found) => {
                            last_drop = Instant::now();
                            self.dropped(vec![found], &prefs, run, r).await;
                        }
                        None => r.event(
                            EventKind::Progress,
                            format!(
                                "{}: {} still to drop",
                                game.name,
                                cards(game.drops.remaining)
                            ),
                        ),
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
                let times = run.kept().set_aside(app_id, Utc::now());
                let what_now = if times >= GIVE_UP_TIMES {
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
    /// together, as one stretch of building hours, until the first of them
    /// does.
    async fn build_hours(
        &self,
        app_ids: Vec<u32>,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let prefs = (self.prefs)();
        if let Err(e) = self.play.play(&app_ids, prefs.appear_online).await {
            return Outcome::Lost(e.to_string());
        }
        if self.play.blocked().is_some() {
            return Outcome::Blocked;
        }
        let stretch = run.kept().start(&app_ids, Mode::Hours, Utc::now());
        let outcome = self.build(&app_ids, prefs, run, r, token).await;
        run.kept().stop(stretch, Utc::now());
        outcome
    }

    /// Steam's badge pages lag behind, so the hours are counted here.
    async fn build(
        &self,
        app_ids: &[u32],
        mut prefs: Preferences,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let lead = run.kept().name(app_ids[0]);
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
            let ready_at = Instant::now() + until_ready(&run.kept().library, app_ids);
            self.report(run, app_ids, Mode::Hours, ready_at, &prefs, r);
            let woke = self.wait(Some(ready_at), token).await;
            run.kept().count(app_ids, counted_to.elapsed());
            counted_to = Instant::now();
            match woke {
                None => return Outcome::Stopped,
                Some(Woke::Signal(signal)) => match signal {
                    Signal::Blocked(_) => return Outcome::Blocked,
                    Signal::Replaced => return Outcome::Replaced,
                    Signal::Lost(why) => return Outcome::Lost(why),
                    Signal::Unblocked => continue,
                    // A card may have dropped for one of them: read again.
                    Signal::NewItems(items) => {
                        run.kept().keep_announced(items);
                        r.event(EventKind::Progress, "Steam says new items arrived".into());
                        tokio::time::sleep(AFTER_NEW_ITEMS).await;
                        return Outcome::Again;
                    }
                },
                Some(Woke::Look | Woke::Tick) => {}
            }
            let ready = {
                let kept = run.kept();
                app_ids
                    .iter()
                    .find_map(|&id| kept.library.game(id).filter(|g| can_drop(g)))
                    .map(|g| g.name.clone())
            };
            if let Some(ready) = ready {
                r.info(format!("{ready} has 3 hours now: its cards can drop."));
                return Outcome::Again;
            }
            let latest = (self.prefs)();
            if latest != prefs {
                let planned = {
                    let kept = run.kept();
                    plan(&kept.library, &latest, &kept.set_aside)
                };
                if planned != Plan::Hours(app_ids.to_vec()) {
                    r.event(
                        EventKind::Switched,
                        "Your choices changed: farming in the new order".into(),
                    );
                    return Outcome::Again;
                }
                if latest.appear_online != prefs.appear_online
                    && let Err(e) = self.play.play(app_ids, latest.appear_online).await
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
        let status = {
            let kept = run.kept();
            FarmingStatus {
                status: Status::Idle,
                library: kept.library.clone(),
                order: Vec::new(),
                next_look: Some(when(until)),
                session: kept.session.clone(),
                set_aside: kept.set_aside.clone(),
                note: why_nothing(&kept.library, &prefs).into(),
                ..Default::default()
            }
        };
        r.status(status);
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
    /// again, so as not to take over from it. Items Steam announces
    /// meanwhile are kept: a card the other device's game drops is this
    /// session's too.
    async fn wait_out(
        &self,
        mut by: Option<u32>,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Waited {
        loop {
            let prefs = (self.prefs)();
            let status = {
                let kept = run.kept();
                FarmingStatus {
                    status: Status::Blocked,
                    library: kept.library.clone(),
                    order: farm_order(&kept.library, &prefs, &kept.set_aside),
                    blocked_by: by,
                    session: kept.session.clone(),
                    set_aside: kept.set_aside.clone(),
                    note: "playing on another device — farming waits until it stops".into(),
                    ..Default::default()
                }
            };
            r.status(status);
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
                    Signal::NewItems(items) => run.kept().keep_announced(items),
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
                    Signal::NewItems(items) => {
                        run.kept().keep_announced(items);
                        return Waited::Resume;
                    }
                    Signal::Unblocked | Signal::Lost(_) => return Waited::Resume,
                },
            }
        }
    }

    /// Tells of the drops a look or read just found, at once, each still
    /// being found out; then which card each was.
    async fn dropped(&self, found: Vec<Found>, prefs: &Preferences, run: &Run, r: &Reporter) {
        if found.is_empty() {
            return;
        }
        let (lines, ask) = {
            let mut kept = run.kept();
            kept.first_forecast(prefs, Utc::now());
            let lines: Vec<String> = found
                .iter()
                .filter_map(|f| {
                    kept.library
                        .game(f.app_id)
                        .map(|g| dropped_message(g, f.count))
                })
                .collect();
            let games: Vec<u32> = found.iter().map(|f| f.app_id).collect();
            (lines, kept.take_announced(&games))
        };
        for line in lines {
            r.event(EventKind::Dropped, line);
        }
        run.tell(r);

        let described = self.describe(ask, r).await;
        let lines = {
            let mut kept = run.kept();
            let named = kept.identify(&found, &described);
            told(&named, &kept)
        };
        for (kind, line) in lines {
            r.event(kind, line);
        }
        run.tell(r);
    }

    /// Asks Steam which cards these items are, if there are any to ask
    /// about. What it can't say, the card page may.
    async fn describe(&self, asset_ids: Vec<u64>, r: &Reporter) -> Vec<CardAsset> {
        if asset_ids.is_empty() {
            return Vec::new();
        }
        r.event(
            EventKind::Progress,
            if asset_ids.len() == 1 {
                "Asking Steam which card it was"
            } else {
                "Asking Steam which cards they were"
            }
            .into(),
        );
        let why = match (self.describe)(asset_ids).await {
            Ok(Ok(cards)) => return cards,
            Ok(Err(e)) => e.to_string(),
            Err(e) => e.to_string(),
        };
        r.event(
            EventKind::Warning,
            format!("Couldn't ask Steam which card it was: {why}"),
        );
        Vec::new()
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
        let status = {
            let kept = run.kept();
            let alone = match mode {
                Mode::Cards => playing.first().and_then(|&id| kept.library.game(id)),
                Mode::Hours => None,
            };
            FarmingStatus {
                status: Status::Farming,
                library: kept.library.clone(),
                order: farm_order(&kept.library, prefs, &kept.set_aside),
                playing: playing.to_vec(),
                mode: Some(mode),
                blocked_by: None,
                next_look: Some(when(next_look)),
                look_every: alone.map(look_every),
                session: kept.session.clone(),
                set_aside: kept.set_aside.clone(),
                note: String::new(),
            }
        };
        r.status(status);
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
fn until_ready(library: &SteamLibrary, app_ids: &[u32]) -> Duration {
    let least = app_ids
        .iter()
        .filter_map(|&id| library.game(id))
        .map(hours_to_go)
        .fold(HOURS_BEFORE_DROPS, f64::min);
    Duration::from_secs_f64((least * 3600.0).max(1.0))
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
fn dropped_message(game: &Game, dropped: usize) -> String {
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

/// What to say of drops just named: which card each was ("Madison dropped
/// for Heavy Rain (a 2nd copy)"), or that it can't be told. A card only the
/// card page could name says so first.
fn told(named: &[Drop], kept: &Kept) -> Vec<(EventKind, String)> {
    let mut lines = Vec::new();
    let mut by_page = Vec::new();
    for drop in named {
        let game = kept.name(drop.app_id);
        let Some(card) = drop.card.name() else {
            lines.push((
                EventKind::Warning,
                format!("Couldn't tell which card dropped for {game}"),
            ));
            continue;
        };
        if matches!(drop.card, DropCard::NameOnly { .. }) && !by_page.contains(&drop.app_id) {
            by_page.push(drop.app_id);
            lines.push((
                EventKind::Progress,
                format!("Steam didn't say which card dropped for {game}: going by its card page"),
            ));
        }
        let foil = if drop.card.is_foil() { " (foil)" } else { "" };
        let copy = match drop.copy {
            Some(copy) if copy > 1 => format!(" (a {} copy)", ordinal(copy)),
            _ => String::new(),
        };
        lines.push((
            EventKind::Identified,
            format!("{card}{foil} dropped for {game}{copy}"),
        ));
    }
    lines
}

/// "2nd", "3rd", "11th", "21st".
fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (_, 11..=13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Why farming moved on from `name` after the user's choices changed.
fn switching(name: &str, app_id: u32, prefs: &Preferences) -> String {
    if prefs.wants(app_id) {
        format!("Moving on from {name}: something is ranked higher now")
    } else {
        format!("Moving on from {name}: it isn't to be farmed now")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_are_counted_as_people_say_them() {
        let said: Vec<String> = [1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 101, 111]
            .into_iter()
            .map(ordinal)
            .collect();
        assert_eq!(
            said,
            [
                "1st", "2nd", "3rd", "4th", "11th", "12th", "13th", "21st", "22nd", "23rd",
                "101st", "111th"
            ]
        );
    }
}
