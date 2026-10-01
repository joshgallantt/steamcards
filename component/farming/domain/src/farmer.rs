//! The farmer: reading the library, playing what the plan says, looking at
//! the cards as they drop, telling which card each was, and stepping aside
//! while another device plays. What a session has seen outlasts a run of the
//! farmer: the session's keeper keeps it until it's ended. A pause stops a
//! run at once, whatever it's waiting for.

use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::Duration,
};

use card::{AssetId, CardAsset, IdentifyCardsUseCase, LookAtCardsUseCase, LookAtFoilsUseCase};
use chrono::Utc;
use preferences::{GetPreferencesUseCase, Preferences};
use session::{DropCard, Found, KeptSession, Mode, SessionKeeper};
use steam_library::{AppId, Game, HOURS_BEFORE_DROPS, ReadLibraryUseCase, SteamLibrary};
use tokio::{sync::mpsc, time::Instant};
use tokio_util::sync::CancellationToken;

use crate::{
    FarmingEvent, FarmingRepository, FarmingStatus, FarmingUpdate, Signal, Status, Trouble,
    ranking::{Plan, farm_order, plan, why_nothing},
    reporter::Reporter,
    rules::{
        AFTER_BLOCK, AFTER_NEW_ITEMS, AFTER_TAKEN_OVER, GIVE_UP_AFTER, GIVE_UP_TIMES, IDLE_LOOK,
        LOOK_EVERY, LOOK_EVERY_LAST, RETRY_CONNECT, RETRY_READ, TICK,
    },
};

/// Farms through the game, card and preferences components' use cases, never
/// their storage, plays through the repository, and writes the session its
/// keeper keeps from one run to the next.
pub(crate) struct Farmer {
    read_library: Arc<dyn ReadLibraryUseCase>,
    look_at_cards: Arc<dyn LookAtCardsUseCase>,
    look_at_foils: Arc<dyn LookAtFoilsUseCase>,
    identify_cards: Arc<dyn IdentifyCardsUseCase>,
    play: Arc<dyn FarmingRepository>,
    get_preferences: Arc<dyn GetPreferencesUseCase>,
    sessions: Arc<SessionKeeper>,
}

/// How a spell of playing ended.
enum Outcome {
    /// Done, or something changed: read the library and plan again.
    Again,
    /// Another device plays: wait until it's done.
    Elsewhere(Elsewhere),
    /// The connection went.
    Lost(String),
    /// Another session took this one's place.
    Replaced,
    Stopped,
}

/// Why farming waits for another device.
#[derive(Clone, Copy)]
enum Elsewhere {
    /// Steam says it's playing: this game, when Steam says.
    Playing(Option<AppId>),
    /// It took over playing, and Steam signed this session off. Its game
    /// may not have started yet.
    TookOver,
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
    kept: Arc<Mutex<KeptSession>>,
}

impl Run {
    fn kept(&self) -> MutexGuard<'_, KeptSession> {
        self.kept.lock().unwrap()
    }

    /// Starts the run's word to the screens from what the session holds, so
    /// its first, while the badges are read, carries the session on.
    fn carry_on(&self, r: &Reporter, prefs: &Preferences) {
        let kept = self.kept();
        let order = farm_order(&kept.library, prefs, &kept.set_aside);
        r.remember(|s| {
            s.library = kept.library.clone();
            s.sets = kept.sets.clone();
            s.order = order;
            s.session = kept.session.clone();
            s.set_aside = kept.set_aside.clone();
        });
    }

    /// Tells the screens what the session holds now.
    fn tell(&self, r: &Reporter) {
        let (library, sets, session, set_aside) = {
            let kept = self.kept();
            (
                kept.library.clone(),
                kept.sets.clone(),
                kept.session.clone(),
                kept.set_aside.clone(),
            )
        };
        r.amend(|s| {
            s.library = library;
            s.sets = sets;
            s.session = session;
            s.set_aside = set_aside;
        });
    }
}

impl Farmer {
    pub(crate) fn new(
        read_library: Arc<dyn ReadLibraryUseCase>,
        look_at_cards: Arc<dyn LookAtCardsUseCase>,
        look_at_foils: Arc<dyn LookAtFoilsUseCase>,
        identify_cards: Arc<dyn IdentifyCardsUseCase>,
        play: Arc<dyn FarmingRepository>,
        get_preferences: Arc<dyn GetPreferencesUseCase>,
        sessions: Arc<SessionKeeper>,
    ) -> Self {
        Self {
            read_library,
            look_at_cards,
            look_at_foils,
            identify_cards,
            play,
            get_preferences,
            sessions,
        }
    }

    /// One run of the farmer: carries on the session the last run left,
    /// farms until the token is cancelled, reporting on the channel, then
    /// stops playing.
    pub(crate) async fn farm(
        &self,
        token: CancellationToken,
        updates: mpsc::Sender<FarmingUpdate>,
    ) {
        let r = Reporter::new(updates);
        let run = Run {
            kept: self.sessions.current(Utc::now()),
        };
        run.carry_on(&r, &self.get_preferences.call());
        self.run(&run, &r, &token).await;
        self.play.stop().await;
    }

    async fn run(&self, run: &Run, r: &Reporter, token: &CancellationToken) {
        loop {
            if token.is_cancelled() {
                return;
            }
            r.stage(Status::Checking);
            let read = tokio::select! {
                _ = token.cancelled() => return,
                read = self.read_library.call() => read,
            };
            match read {
                Ok(Ok(fresh)) => {
                    let prefs = self.get_preferences.call();
                    let found = run.kept().take(
                        fresh,
                        |library, aside| farm_order(library, &prefs, aside),
                        Utc::now(),
                    );
                    self.dropped(found, &prefs, run, r, token).await;
                    if token.is_cancelled() {
                        return;
                    }
                }
                failed => {
                    let why = match failed {
                        Ok(Err(e)) => e.to_string(),
                        Err(e) => e.to_string(),
                        Ok(Ok(_)) => String::new(),
                    };
                    let trouble = Trouble::BadgesUnread(why);
                    r.event(FarmingEvent::WentWrong {
                        trouble: trouble.clone(),
                        again_in: Some(RETRY_READ),
                    });
                    r.stage_trouble(trouble, Some(when(Instant::now() + RETRY_READ)));
                    if !pause(RETRY_READ, token).await {
                        return;
                    }
                    continue;
                }
            }
            let prefs = self.get_preferences.call();
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
                Outcome::Again => {}
                Outcome::Elsewhere(why) => match self.wait_out(why, run, r, token).await {
                    Waited::Resume => {}
                    Waited::Replaced => return replaced(r),
                    Waited::Stopped => return,
                },
                Outcome::Stopped => return,
                Outcome::Replaced => return replaced(r),
                Outcome::Lost(why) => {
                    if !lost(Trouble::Lost(why), r, token).await {
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
        app_id: AppId,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let prefs = self.get_preferences.call();
        if let Err(e) = self.play.play(&[app_id], prefs.appear_online).await {
            return Outcome::Lost(e.to_string());
        }
        if let Some(by) = self.play.blocked() {
            return Outcome::Elsewhere(Elsewhere::Playing(by));
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
        r.event(FarmingEvent::FarmingCards {
            game: game.name.clone(),
            cards_left: game.drops.remaining,
        });
        let mut last_drop = Instant::now();
        // A set not read yet is looked at first, as ASF looks at a game as it
        // starts on it: then a card that drops can be told by the set's
        // counts, and which copy it is.
        let set_known = run.kept().sets.set(app_id).is_some_and(|s| !s.is_empty());
        let mut next_look = if !set_known {
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
                    Signal::Blocked(by) => return Outcome::Elsewhere(Elsewhere::Playing(by)),
                    Signal::TakenOver => return Outcome::Elsewhere(Elsewhere::TookOver),
                    Signal::Replaced => return Outcome::Replaced,
                    Signal::Lost(why) => return Outcome::Lost(why),
                    Signal::Unblocked => continue,
                    Signal::NewItems(items) => {
                        run.kept().keep_announced(items);
                        r.event(FarmingEvent::NewItems);
                        if !pause(AFTER_NEW_ITEMS, token).await {
                            return Outcome::Stopped;
                        }
                    }
                },
                Some(Woke::Tick) => {
                    let latest = self.get_preferences.call();
                    if latest == prefs {
                        continue;
                    }
                    let planned = {
                        let kept = run.kept();
                        plan(&kept.library, &latest, &kept.set_aside)
                    };
                    if planned != Plan::Cards(app_id) {
                        r.event(FarmingEvent::MovedOn {
                            game: game.name.clone(),
                            outranked: latest.wants(app_id),
                        });
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

            let looked = tokio::select! {
                _ = token.cancelled() => return Outcome::Stopped,
                looked = self.look_at_cards.call(app_id) => looked,
            };
            match looked {
                Ok(Ok(fresh)) => {
                    let looked = run.kept().update(fresh, Utc::now());
                    game = looked.game;
                    match looked.found {
                        Some(found) => {
                            last_drop = Instant::now();
                            self.dropped(vec![found], &prefs, run, r, token).await;
                        }
                        None => {
                            r.event(FarmingEvent::Looked {
                                game: game.name.clone(),
                                cards_left: game.drops.remaining,
                            });
                            if run.kept().has_items_for(app_id) {
                                self.described_late(app_id, run, r, token).await;
                            }
                        }
                    }
                    if !game.has_drops_left() {
                        r.event(FarmingEvent::AllDropped { game: game.name });
                        return Outcome::Again;
                    }
                }
                Ok(Err(e)) => r.event(FarmingEvent::CardsUnread {
                    game: game.name.clone(),
                    why: e.to_string(),
                }),
                Err(e) => r.event(FarmingEvent::CardsUnread {
                    game: game.name.clone(),
                    why: e.to_string(),
                }),
            }

            if last_drop.elapsed() >= GIVE_UP_AFTER {
                let times = run.kept().set_aside(app_id, Utc::now());
                r.event(FarmingEvent::Stalled {
                    game: game.name,
                    after: GIVE_UP_AFTER,
                    for_good: times >= GIVE_UP_TIMES,
                });
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
        app_ids: Vec<AppId>,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        let prefs = self.get_preferences.call();
        if let Err(e) = self.play.play(&app_ids, prefs.appear_online).await {
            return Outcome::Lost(e.to_string());
        }
        if let Some(by) = self.play.blocked() {
            return Outcome::Elsewhere(Elsewhere::Playing(by));
        }
        let stretch = run.kept().start(&app_ids, Mode::Hours, Utc::now());
        let outcome = self.build(&app_ids, prefs, run, r, token).await;
        run.kept().stop(stretch, Utc::now());
        outcome
    }

    /// Steam's badge pages lag behind, so the hours are counted here.
    async fn build(
        &self,
        app_ids: &[AppId],
        mut prefs: Preferences,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Outcome {
        r.event(FarmingEvent::BuildingHours {
            lead: run.kept().name(app_ids[0]),
            games: app_ids.len(),
        });
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
                    Signal::Blocked(by) => return Outcome::Elsewhere(Elsewhere::Playing(by)),
                    Signal::TakenOver => return Outcome::Elsewhere(Elsewhere::TookOver),
                    Signal::Replaced => return Outcome::Replaced,
                    Signal::Lost(why) => return Outcome::Lost(why),
                    Signal::Unblocked => continue,
                    // A card may have dropped for one of them: read again.
                    Signal::NewItems(items) => {
                        run.kept().keep_announced(items);
                        r.event(FarmingEvent::NewItems);
                        if !pause(AFTER_NEW_ITEMS, token).await {
                            return Outcome::Stopped;
                        }
                        return Outcome::Again;
                    }
                },
                Some(Woke::Look | Woke::Tick) => {}
            }
            let ready = {
                let kept = run.kept();
                app_ids
                    .iter()
                    .find_map(|&id| kept.library.game(id).filter(|g| g.can_drop()))
                    .map(|g| g.name.clone())
            };
            if let Some(ready) = ready {
                r.event(FarmingEvent::HoursBuilt { game: ready });
                return Outcome::Again;
            }
            let latest = self.get_preferences.call();
            if latest != prefs {
                let planned = {
                    let kept = run.kept();
                    plan(&kept.library, &latest, &kept.set_aside)
                };
                if planned != Plan::Hours(app_ids.to_vec()) {
                    r.event(FarmingEvent::ChoicesChanged);
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
        let prefs = self.get_preferences.call();
        let until = Instant::now() + IDLE_LOOK;
        let status = {
            let kept = run.kept();
            FarmingStatus {
                status: Status::Idle,
                library: kept.library.clone(),
                sets: kept.sets.clone(),
                order: Vec::new(),
                next_look: Some(when(until)),
                session: kept.session.clone(),
                set_aside: kept.set_aside.clone(),
                nothing_to_farm: Some(why_nothing(&kept.library, &prefs)),
                ..Default::default()
            }
        };
        r.status(status);
        loop {
            tokio::select! {
                _ = token.cancelled() => return Outcome::Stopped,
                _ = tokio::time::sleep_until(until) => return Outcome::Again,
                _ = tokio::time::sleep(TICK) => {
                    if self.get_preferences.call() != prefs {
                        return Outcome::Again;
                    }
                }
            }
        }
    }

    /// Waits while another device plays, then a while more before playing
    /// again, so as not to get in its way: a minute after it stops or, when
    /// it took over and Steam doesn't say it's playing yet, a few minutes for
    /// its game to start. Signed on all the while, playing nothing, so Steam
    /// can say when it stops. Items Steam announces meanwhile are kept: a
    /// card the other device's game drops is this session's too.
    async fn wait_out(
        &self,
        why: Elsewhere,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Waited {
        r.event(waiting_for(why, &run.kept()));
        let mut grace = match why {
            Elsewhere::Playing(_) => AFTER_BLOCK,
            Elsewhere::TookOver => AFTER_TAKEN_OVER,
        };
        'listening: loop {
            if let Err(e) = self.play.listen().await {
                if !lost(Trouble::Lost(e.to_string()), r, token).await {
                    return Waited::Stopped;
                }
                continue;
            }
            if let Some(by) = self.play.blocked() {
                self.waiting(by, None, run, r);
                loop {
                    let signal = tokio::select! {
                        _ = token.cancelled() => return Waited::Stopped,
                        signal = self.play.next_signal() => signal,
                    };
                    let trouble = match signal {
                        Signal::Unblocked => break,
                        Signal::Blocked(by) => {
                            self.waiting(by, None, run, r);
                            continue;
                        }
                        Signal::NewItems(items) => {
                            run.kept().keep_announced(items);
                            continue;
                        }
                        Signal::Replaced => return Waited::Replaced,
                        Signal::Lost(why) => Trouble::Lost(why),
                        Signal::TakenOver => Trouble::SignedOff,
                    };
                    // Signing on again says whether it still plays.
                    if !lost(trouble, r, token).await {
                        return Waited::Stopped;
                    }
                    continue 'listening;
                }
                grace = AFTER_BLOCK;
                r.event(FarmingEvent::ElsewhereStopped { carry_on_in: grace });
            }
            // A while more, in case it plays again.
            let until = Instant::now() + grace;
            self.waiting(None, Some(until), run, r);
            loop {
                let signal = tokio::select! {
                    _ = token.cancelled() => return Waited::Stopped,
                    _ = tokio::time::sleep_until(until) => return Waited::Resume,
                    signal = self.play.next_signal() => signal,
                };
                match signal {
                    Signal::Blocked(_) => continue 'listening,
                    Signal::NewItems(items) => run.kept().keep_announced(items),
                    Signal::Replaced => return Waited::Replaced,
                    // Nothing to do until the while is up: playing then signs
                    // on again if need be, and plays nothing while another
                    // device does.
                    Signal::Unblocked | Signal::Lost(_) | Signal::TakenOver => {}
                }
            }
        }
    }

    /// Tells the screens farming waits for another device: while it plays
    /// `by` (when Steam says what), or once it's done, until `until`.
    fn waiting(&self, by: Option<AppId>, until: Option<Instant>, run: &Run, r: &Reporter) {
        let prefs = self.get_preferences.call();
        let status = {
            let kept = run.kept();
            FarmingStatus {
                status: Status::Blocked,
                library: kept.library.clone(),
                sets: kept.sets.clone(),
                order: farm_order(&kept.library, &prefs, &kept.set_aside),
                blocked_by: by,
                next_look: until.map(when),
                session: kept.session.clone(),
                set_aside: kept.set_aside.clone(),
                ..Default::default()
            }
        };
        r.status(status);
    }

    /// Tells of the drops a look or read just found, at once, each still
    /// being found out; then which card each was, and which copy. A read
    /// shows no sets, so each game it found drops for is looked at first:
    /// its card page tells them, and keeps its set right for the next. A
    /// pause meanwhile tells what's known by then, and stops.
    async fn dropped(
        &self,
        mut found: Vec<Found>,
        prefs: &Preferences,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) {
        if found.is_empty() {
            return;
        }
        let (events, ask) = {
            let mut kept = run.kept();
            kept.first_forecast(
                |library, aside| farm_order(library, prefs, aside),
                Utc::now(),
            );
            let events: Vec<FarmingEvent> = found
                .iter()
                .filter_map(|f| {
                    kept.library
                        .game(f.app_id)
                        .map(|g| dropped_for(g, f.count()))
                })
                .collect();
            let games: Vec<AppId> = found.iter().map(|f| f.app_id).collect();
            (events, kept.take_announced(&games))
        };
        for event in events {
            r.event(event);
        }
        run.tell(r);

        for f in found.iter_mut().filter(|f| f.needs_look()) {
            let looked = tokio::select! {
                _ = token.cancelled() => break,
                looked = self.look_at_cards.call(f.app_id) => looked,
            };
            let why = match looked {
                Ok(Ok(fresh)) => {
                    let looked = run.kept().update(fresh, Utc::now());
                    if let Some(more) = &looked.found {
                        r.event(dropped_for(&looked.game, more.count()));
                    }
                    f.join(looked);
                    continue;
                }
                Ok(Err(e)) => e.to_string(),
                Err(e) => e.to_string(),
            };
            let game = run.kept().name(f.app_id);
            r.event(FarmingEvent::CardsUnread { game, why });
        }

        let asked = ask.iter().map(|i| i.asset_id).collect();
        let described = match self.describe(asked, r, token).await {
            Some(cards) => cards,
            // KeptSession to ask about again: each may yet name a drop the card
            // page named.
            None => {
                run.kept().keep_announced(ask);
                Vec::new()
            }
        };
        let told = run.kept().identify(&found, &described);
        let with_foils = run.kept().foil_games(&told);
        for app_id in with_foils {
            let foils = tokio::select! {
                _ = token.cancelled() => break,
                foils = self.look_at_foils.call(app_id) => foils,
            };
            let why = match foils {
                Ok(Ok(foils)) => {
                    run.kept().number_foils(app_id, &told, &foils);
                    continue;
                }
                Ok(Err(e)) => e.to_string(),
                Err(e) => e.to_string(),
            };
            let game = run.kept().name(app_id);
            r.event(FarmingEvent::FoilsUnread { game, why });
        }
        let events = which_cards(&told, &run.kept());
        for event in events {
            r.event(event);
        }
        run.tell(r);
    }

    /// Asks Steam about items it announced for a game after its drops were
    /// told: a drop only its card page could name is then named by its own
    /// item.
    async fn described_late(
        &self,
        app_id: AppId,
        run: &Run,
        r: &Reporter,
        token: &CancellationToken,
    ) {
        let ask = run.kept().take_announced(&[app_id]);
        let asked = ask.iter().map(|i| i.asset_id).collect();
        match self.describe(asked, r, token).await {
            Some(cards) => {
                run.kept().identify(&[], &cards);
                run.tell(r);
            }
            None => run.kept().keep_announced(ask),
        }
    }

    /// Asks Steam which cards these items are, if there are any to ask
    /// about. What it can't say, the card page may. `None` when Steam didn't
    /// say, or farming stopped first.
    async fn describe(
        &self,
        asset_ids: Vec<AssetId>,
        r: &Reporter,
        token: &CancellationToken,
    ) -> Option<Vec<CardAsset>> {
        if asset_ids.is_empty() {
            return Some(Vec::new());
        }
        if token.is_cancelled() {
            return None;
        }
        r.event(FarmingEvent::Asking {
            items: asset_ids.len(),
        });
        let answer = tokio::select! {
            _ = token.cancelled() => return None,
            answer = self.identify_cards.call(asset_ids) => answer,
        };
        let why = match answer {
            Ok(Ok(cards)) => return Some(cards),
            Ok(Err(e)) => e.to_string(),
            Err(e) => e.to_string(),
        };
        r.event(FarmingEvent::AskFailed { why });
        None
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
        playing: &[AppId],
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
                sets: kept.sets.clone(),
                order: farm_order(&kept.library, prefs, &kept.set_aside),
                playing: playing.to_vec(),
                mode: Some(mode),
                blocked_by: None,
                next_look: Some(when(next_look)),
                look_every: alone.map(look_every),
                session: kept.session.clone(),
                set_aside: kept.set_aside.clone(),
                nothing_to_farm: None,
                trouble: None,
            }
        };
        r.status(status);
    }
}

/// Says Steam was lost, and why, and waits a minute before trying again;
/// false when stopped meanwhile.
async fn lost(trouble: Trouble, r: &Reporter, token: &CancellationToken) -> bool {
    r.event(FarmingEvent::WentWrong {
        trouble: trouble.clone(),
        again_in: Some(RETRY_CONNECT),
    });
    r.stage_trouble(trouble, Some(when(Instant::now() + RETRY_CONNECT)));
    pause(RETRY_CONNECT, token).await
}

/// What happened, as farming starts waiting for another device.
fn waiting_for(why: Elsewhere, kept: &KeptSession) -> FarmingEvent {
    match why {
        Elsewhere::TookOver => FarmingEvent::TakenOver,
        Elsewhere::Playing(by) => FarmingEvent::PlayedElsewhere {
            game: by
                .and_then(|id| kept.library.game(id))
                .map(|g| g.name.clone()),
        },
    }
}

/// Stops for good: another session signed on in this one's place.
fn replaced(r: &Reporter) {
    r.event(FarmingEvent::WentWrong {
        trouble: Trouble::Replaced,
        again_in: None,
    });
    r.stage_trouble(Trouble::Replaced, None);
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
fn until_ready(library: &SteamLibrary, app_ids: &[AppId]) -> Duration {
    let least = app_ids
        .iter()
        .filter_map(|&id| library.game(id))
        .map(Game::hours_to_go)
        .fold(HOURS_BEFORE_DROPS, f64::min);
    Duration::from_secs_f64((least * 3600.0).max(1.0))
}

/// A tokio instant as a time on the clock, for the screen.
fn when(at: Instant) -> chrono::DateTime<Utc> {
    let left = at.saturating_duration_since(Instant::now());
    Utc::now() + chrono::Duration::from_std(left).unwrap_or_default()
}

/// Cards that dropped for `game`, `count` at once: its drops count them.
fn dropped_for(game: &Game, count: usize) -> FarmingEvent {
    FarmingEvent::Dropped {
        game: game.name.clone(),
        count,
        left: game.drops.remaining,
    }
}

/// Which card each drop just told was, and which copy, or that it can't be
/// told. A card only the card page could name says so first.
fn which_cards(told: &[usize], kept: &KeptSession) -> Vec<FarmingEvent> {
    let mut events = Vec::new();
    let mut by_page = Vec::new();
    for drop in told.iter().filter_map(|&i| kept.session.drops.get(i)) {
        let game = kept.name(drop.app_id);
        let Some(card) = drop.card.name() else {
            events.push(FarmingEvent::Untold { game });
            continue;
        };
        if matches!(drop.card, DropCard::NameOnly { .. }) && !by_page.contains(&drop.app_id) {
            by_page.push(drop.app_id);
            events.push(FarmingEvent::ByCardPage { game: game.clone() });
        }
        events.push(FarmingEvent::Identified {
            game,
            card: card.to_owned(),
            foil: drop.card.is_foil(),
            copy: drop.copy,
        });
    }
    events
}
