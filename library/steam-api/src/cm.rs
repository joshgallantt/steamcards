//! A connection to a Steam CM server: the part of Steam a client signs on to,
//! tells what it's playing, and hears from. Over a WebSocket, as the Steam
//! client does.
//!
//! One task reads, one writes, and once signed on, one sends a heartbeat. A
//! request that expects an answer (a "job") waits for the message that names
//! its job ID. The tasks stop when the `Connection` is dropped, or when Steam
//! closes it.

use std::{
    collections::HashMap,
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{anyhow, bail};
use debug_log::DebugLog;
use futures::{SinkExt, StreamExt};
use prost::Message;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_util::sync::{CancellationToken, DropGuard};

use crate::{
    DEVICE_NAME, EResult,
    inventory::{COMMUNITY_CONTEXT, STEAM_APP},
    packet::{Packet, unpack_multi},
    proto::{
        self, ClientChangeStatus, ClientGamesPlayed, ClientHeartBeat, ClientHello,
        ClientItemAnnouncements, ClientLogOff, ClientLoggedOff, ClientLogon, ClientLogonResponse,
        ClientPlayingSessionState, ClientRequestItemAnnouncements, ClientWalletInfoUpdate,
        GamePlayed, Header, IpAddress, PERSONA_OFFLINE, PERSONA_ONLINE, PROTOCOL_VERSION, emsg,
    },
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const CALL_TIMEOUT: Duration = Duration::from_secs(15);
const LOGON_TIMEOUT: Duration = Duration::from_secs(30);
/// How often to say "still here" when a logon response doesn't say.
const HEARTBEAT: Duration = Duration::from_secs(9);
/// A job ID that means "no job", as Steam's .proto files default it.
const NO_JOB: u64 = u64::MAX;

/// The most games Steam counts as played at once.
pub const MAX_GAMES_PLAYED: usize = 32;

/// What steamcards tells Steam it's running on, as SteamKit's `EOSType`
/// names it: `MacOSUnknown`, `LinuxUnknown`, or `Windows10` anywhere else.
pub(crate) const OS_TYPE: i32 = if cfg!(target_os = "macos") {
    -102
} else if cfg!(target_os = "linux") {
    -203
} else {
    16
};

/// Something Steam said that matters to a farmer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Another device playing on this account blocks this session from
    /// playing, or no longer does.
    PlayingBlocked(Blocked),
    /// Steam announced the account's new items. A card may have dropped.
    NewItems(Announcement),
    /// Steam signed this session off. `LoggedInElsewhere` when another
    /// device took over playing, or when this one said it was playing while
    /// another device was.
    LoggedOff(EResult),
    /// The connection closed.
    Closed,
}

/// Steam's word on the account's new items: how many, and which, when it
/// lists them. It counts and lists an item until the inventory is viewed,
/// so an item comes again with the next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    pub count: u32,
    pub items: Vec<UnseenItem>,
    /// Steam's answer to asking at sign-on: what was new already, before
    /// this session.
    pub at_sign_on: bool,
}

/// An item Steam lists as new.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnseenItem {
    pub asset_id: u64,
    /// The app whose inventory holds it: 753, Steam's own, for community
    /// items such as trading cards.
    pub app_id: u32,
    pub context_id: u64,
    /// When it arrived, in seconds since 1970, when Steam says.
    pub gained_at: Option<u32>,
    /// The game it came from, when Steam says: a card's is the game whose
    /// set it's from.
    pub source_app_id: Option<u32>,
}

impl UnseenItem {
    /// Whether it's one of the account's community items, which
    /// [`SteamClient::describe_items`](crate::SteamClient::describe_items) can
    /// describe.
    pub fn is_community_item(&self) -> bool {
        (self.app_id, self.context_id) == (STEAM_APP, COMMUNITY_CONTEXT)
    }
}

/// Whether another device's game stops this session from playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {
    pub blocked: bool,
    /// What the other device is playing, when Steam says.
    pub app_id: Option<u32>,
}

/// The account's Steam wallet, as Steam says when a session signs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalletInfo {
    pub has_wallet: bool,
    /// Its currency, as an `ECurrency` id: 1 for dollars, 2 for pounds. 0
    /// when the account has no wallet.
    pub currency: i32,
}

impl WalletInfo {
    /// The currency the account's prices are in, as an `ECurrency` id: the
    /// wallet's, or dollars for an account with no wallet, as Valve's pages
    /// ask the market when there's no wallet currency.
    pub fn priced_in(&self) -> u32 {
        const DOLLARS: u32 = 1;
        u32::try_from(self.currency)
            .ok()
            .filter(|&id| self.has_wallet && id > 0)
            .unwrap_or(DOLLARS)
    }
}

/// Steam answered a request with anything but OK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub eresult: EResult,
    /// Steam's own words, when it sent some.
    pub message: String,
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.message.is_empty() {
            write!(f, "Steam said {}", self.eresult)
        } else {
            write!(f, "Steam said {}: {}", self.eresult, self.message)
        }
    }
}

impl std::error::Error for Refused {}

/// Steam didn't answer a call in time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoAnswer;

impl fmt::Display for NoAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Steam didn't answer in time")
    }
}

impl std::error::Error for NoAnswer {}

/// What signing on with a refresh token takes.
#[derive(Debug, Clone)]
pub struct LogOn<'a> {
    pub refresh_token: &'a str,
    pub account_name: &'a str,
    pub steam_id: u64,
    pub login_id: u32,
}

#[derive(Debug, Clone, Copy)]
struct SignedOn {
    steam_id: u64,
    session_id: i32,
}

/// How long Steam's answers as a session signs on may take: whether another
/// device is playing, and what's new. They come within a round trip when
/// they come at all.
pub(crate) const SIGN_ON_ANSWER_WITHIN: Duration = Duration::from_secs(10);

/// What the tasks share.
struct State {
    /// Jobs waiting for their answer.
    pending: Mutex<HashMap<u64, oneshot::Sender<Packet>>>,
    /// A logon waiting for its answer.
    logon: Mutex<Option<oneshot::Sender<Packet>>>,
    signed_on: Mutex<Option<SignedOn>>,
    blocked: Mutex<Option<Blocked>>,
    /// When it asked at sign-on what's new, while not answered yet.
    asked_new_items: Mutex<Option<Instant>>,
    /// How long Steam's answers at sign-on may take.
    answer_within: Mutex<Duration>,
    /// The answer, once it came.
    new_at_sign_on: Mutex<Option<Announcement>>,
    wallet: Mutex<Option<WalletInfo>>,
    /// Why Steam signed the session off, once it has.
    logged_off: Mutex<Option<EResult>>,
    events: broadcast::Sender<Event>,
    next_job: AtomicU64,
    log: DebugLog,
}

pub struct Connection {
    out: mpsc::UnboundedSender<Vec<u8>>,
    state: Arc<State>,
    stop: CancellationToken,
    _stop_when_dropped: DropGuard,
}

impl Connection {
    /// Waits for Steam's answers at sign-on only `within`: for tests that
    /// can't wait.
    pub fn sign_on_answer_within(&self, within: Duration) {
        *self.state.answer_within.lock().unwrap() = within;
    }

    /// Connects to the CM server at `url` (`wss://…/cmsocket/`) and says
    /// hello. Not signed on yet: sign-in calls work, playing doesn't.
    pub async fn connect(url: &str, log: &DebugLog) -> anyhow::Result<Self> {
        log.line(&format!("connecting to {url}"));
        let (ws, _) = tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(url))
            .await
            .map_err(|_| anyhow!("Steam didn't answer in time"))?
            .map_err(|e| anyhow!("couldn't reach Steam ({e})"))?;
        let (mut sink, mut stream) = ws.split();
        let (out, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
        let stop = CancellationToken::new();
        let (events, _) = broadcast::channel(64);
        let state = Arc::new(State {
            pending: Mutex::default(),
            logon: Mutex::default(),
            signed_on: Mutex::default(),
            blocked: Mutex::default(),
            asked_new_items: Mutex::default(),
            answer_within: Mutex::new(SIGN_ON_ANSWER_WITHIN),
            new_at_sign_on: Mutex::default(),
            wallet: Mutex::default(),
            logged_off: Mutex::default(),
            events,
            next_job: AtomicU64::new(1),
            log: log.clone(),
        });

        let writing = stop.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = writing.cancelled() => break,
                    frame = outgoing.recv() => match frame {
                        Some(frame) => {
                            if sink.send(Frame::Binary(frame.into())).await.is_err() {
                                break;
                            }
                        }
                        None => break,
                    },
                }
            }
            let _ = sink.close().await;
        });

        let reading = stop.clone();
        let shared = state.clone();
        tokio::spawn(async move {
            loop {
                let frame = tokio::select! {
                    _ = reading.cancelled() => break,
                    frame = stream.next() => frame,
                };
                match frame {
                    Some(Ok(Frame::Binary(bytes))) => shared.receive(&bytes),
                    Some(Ok(Frame::Close(_))) | None => break,
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        shared.log.line(&format!("connection dropped: {e}"));
                        break;
                    }
                }
            }
            shared.close();
            reading.cancel();
        });

        let conn = Self {
            out,
            state,
            _stop_when_dropped: stop.clone().drop_guard(),
            stop,
        };
        conn.send(
            emsg::CLIENT_HELLO,
            Header::default(),
            &ClientHello {
                protocol_version: Some(PROTOCOL_VERSION),
            },
        )?;
        Ok(conn)
    }

    /// Signs on with a refresh token, as the Steam client does, and starts
    /// the heartbeat. Errs with [`Refused`] when Steam says no.
    pub async fn log_on(&self, details: &LogOn<'_>) -> anyhow::Result<()> {
        let (tx, rx) = oneshot::channel();
        *self.state.logon.lock().unwrap() = Some(tx);
        let header = Header {
            steamid: Some(details.steam_id),
            client_sessionid: Some(0),
            ..Default::default()
        };
        let body = ClientLogon {
            protocol_version: Some(PROTOCOL_VERSION),
            client_package_version: Some(1771),
            client_language: Some("english".into()),
            client_os_type: Some(OS_TYPE as u32),
            should_remember_password: Some(true),
            obfuscated_private_ip: Some(IpAddress {
                v4: Some(details.login_id),
            }),
            // The new Steam chat, as today's client says.
            chat_mode: Some(2),
            account_name: Some(details.account_name.to_owned()),
            machine_name: Some(DEVICE_NAME.to_owned()),
            access_token: Some(details.refresh_token.to_owned()),
        };
        self.send(emsg::CLIENT_LOGON, header, &body)?;
        let packet = tokio::time::timeout(LOGON_TIMEOUT, rx)
            .await
            .map_err(|_| anyhow!("Steam didn't answer the sign-on in time"))?
            .map_err(|_| anyhow!("the connection to Steam closed"))?;
        let answer: ClientLogonResponse = proto::decode("logon response", &packet.body)?;
        let eresult = EResult::of(answer.eresult);
        self.state.log.line(&format!("signing on: {eresult}"));
        if !eresult.is_ok() {
            return Err(Refused {
                eresult,
                message: String::new(),
            }
            .into());
        }
        *self.state.signed_on.lock().unwrap() = Some(SignedOn {
            steam_id: packet
                .header
                .steamid
                .filter(|&id| id != 0)
                .unwrap_or(details.steam_id),
            session_id: packet.header.client_sessionid.unwrap_or_default(),
        });
        let every = [
            answer.heartbeat_seconds,
            answer.legacy_out_of_game_heartbeat_seconds,
        ]
        .into_iter()
        .flatten()
        .find(|&s| s > 0)
        .map_or(HEARTBEAT, |s| Duration::from_secs(s as u64));
        self.heartbeat(every);
        // What's new in the inventory already, as ASF and node-steam-user
        // ask at sign-on: the answer tells new items from those before.
        *self.state.asked_new_items.lock().unwrap() = Some(Instant::now());
        self.send(
            emsg::CLIENT_REQUEST_ITEM_ANNOUNCEMENTS,
            self.header(),
            &ClientRequestItemAnnouncements {},
        )?;
        Ok(())
    }

    /// Tells Steam to count these games as played, and only these: none stops
    /// playing. Steam counts at most [`MAX_GAMES_PLAYED`]. While another
    /// device plays, Steam signs off a session that says it's playing: see
    /// [`playing_state`](Self::playing_state).
    pub fn play(&self, app_ids: &[u32]) -> anyhow::Result<()> {
        if !self.is_signed_on() {
            bail!("not signed on to Steam");
        }
        let body = ClientGamesPlayed {
            games_played: app_ids
                .iter()
                .take(MAX_GAMES_PLAYED)
                .map(|&id| GamePlayed {
                    game_id: Some(u64::from(id)),
                })
                .collect(),
            client_os_type: Some(OS_TYPE as u32),
        };
        self.state.log.line(&format!("playing {app_ids:?}"));
        self.send(
            emsg::CLIENT_GAMES_PLAYED_WITH_DATA_BLOB,
            self.header(),
            &body,
        )
    }

    /// Shows this session to friends as online, or not. A session is offline
    /// until it says otherwise, and Steam counts what it plays either way:
    /// offline, friends don't see the games.
    pub fn set_online(&self, online: bool) -> anyhow::Result<()> {
        if !self.is_signed_on() {
            bail!("not signed on to Steam");
        }
        let persona_state = if online {
            PERSONA_ONLINE
        } else {
            PERSONA_OFFLINE
        };
        self.state.log.line(&format!(
            "appearing {}",
            if online { "online" } else { "offline" }
        ));
        self.send(
            emsg::CLIENT_CHANGE_STATUS,
            self.header(),
            &ClientChangeStatus {
                persona_state: Some(persona_state),
            },
        )
    }

    /// Signs off and closes, giving Steam a moment to hear it.
    pub async fn log_off(&self) {
        if self.is_signed_on() {
            let _ = self.send(emsg::CLIENT_LOG_OFF, self.header(), &ClientLogOff {});
            let _ = tokio::time::timeout(Duration::from_secs(2), self.stop.cancelled()).await;
        }
        self.stop.cancel();
    }

    /// What Steam says from here on.
    pub fn events(&self) -> broadcast::Receiver<Event> {
        self.state.events.subscribe()
    }

    /// Whether another device's game blocks this session from playing, as
    /// Steam last said; `None` until it says.
    pub fn blocked(&self) -> Option<Blocked> {
        *self.state.blocked.lock().unwrap()
    }

    /// The same, waiting for Steam to say if it hasn't yet: it says within
    /// moments of a session signing on. `None` if it doesn't say in the time
    /// a sign-on answer may take, or the connection closes first.
    pub async fn playing_state(&self) -> Option<Blocked> {
        // Listening first, so what's said meanwhile isn't missed.
        let mut events = self.events();
        if let Some(said) = self.blocked() {
            return Some(said);
        }
        let within = *self.state.answer_within.lock().unwrap();
        let said = async {
            loop {
                match events.recv().await {
                    Ok(Event::PlayingBlocked(said)) => return Some(said),
                    Ok(Event::LoggedOff(_) | Event::Closed)
                    | Err(broadcast::error::RecvError::Closed) => return None,
                    Ok(Event::NewItems(_)) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if let Some(said) = self.blocked() {
                            return Some(said);
                        }
                    }
                }
            }
        };
        tokio::time::timeout(within, said).await.ok().flatten()
    }

    /// The new items Steam listed when asked at sign-on: those already there
    /// before this session. `None` until it answers.
    pub fn new_at_sign_on(&self) -> Option<Announcement> {
        self.state.new_at_sign_on.lock().unwrap().clone()
    }

    /// The account's wallet, as Steam said when this session signed on;
    /// `None` until it has.
    pub fn wallet(&self) -> Option<WalletInfo> {
        *self.state.wallet.lock().unwrap()
    }

    pub fn is_signed_on(&self) -> bool {
        !self.is_closed() && self.state.signed_on.lock().unwrap().is_some()
    }

    /// Why Steam signed the session off, if it has: kept after the
    /// connection closes.
    pub(crate) fn logged_off(&self) -> Option<EResult> {
        *self.state.logged_off.lock().unwrap()
    }

    pub fn is_closed(&self) -> bool {
        self.stop.is_cancelled()
    }

    /// The account signed on, once it is.
    pub fn steam_id(&self) -> Option<u64> {
        self.state.signed_on.lock().unwrap().map(|s| s.steam_id)
    }

    /// Calls a service method (`"Authentication.PollAuthSessionStatus#1"`)
    /// and reads its answer. Errs with [`Refused`] when Steam says no, and
    /// [`NoAnswer`] when it doesn't answer in time.
    pub(crate) async fn call<Req: Message, Resp: Message + Default>(
        &self,
        method: &str,
        req: &Req,
    ) -> anyhow::Result<Resp> {
        let job = self.state.next_job.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.state.pending.lock().unwrap().insert(job, tx);
        let emsg = if self.state.signed_on.lock().unwrap().is_some() {
            emsg::SERVICE_METHOD_CALL_FROM_CLIENT
        } else {
            emsg::SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED
        };
        let header = Header {
            jobid_source: Some(job),
            target_job_name: Some(method.to_owned()),
            ..self.header()
        };
        let answer = match self.send(emsg, header, req) {
            Ok(()) => tokio::time::timeout(CALL_TIMEOUT, rx).await,
            Err(e) => {
                self.state.pending.lock().unwrap().remove(&job);
                return Err(e);
            }
        };
        self.state.pending.lock().unwrap().remove(&job);
        let packet = answer
            .map_err(|_| NoAnswer)?
            .map_err(|_| anyhow!("the connection to Steam closed"))?;
        let eresult = EResult::of(packet.header.eresult);
        if !eresult.is_ok() {
            self.state.log.line(&format!("{method}: {eresult}"));
            return Err(Refused {
                eresult,
                message: packet.header.error_message.unwrap_or_default(),
            }
            .into());
        }
        proto::decode(method, &packet.body)
    }

    /// The header every message after signing on carries.
    fn header(&self) -> Header {
        match *self.state.signed_on.lock().unwrap() {
            Some(s) => Header {
                steamid: Some(s.steam_id),
                client_sessionid: Some(s.session_id),
                ..Default::default()
            },
            None => Header::default(),
        }
    }

    fn send(&self, emsg: u32, header: Header, body: &impl Message) -> anyhow::Result<()> {
        self.out
            .send(Packet::encode(emsg, &header, body))
            .map_err(|_| anyhow!("the connection to Steam closed"))
    }

    fn heartbeat(&self, every: Duration) {
        let out = self.out.clone();
        let stop = self.stop.clone();
        let beat = Packet::encode(
            emsg::CLIENT_HEART_BEAT,
            &self.header(),
            &ClientHeartBeat { send_reply: None },
        );
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(every);
            tick.tick().await;
            loop {
                tokio::select! {
                    _ = stop.cancelled() => break,
                    _ = tick.tick() => {
                        if out.send(beat.clone()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Connection")
            .field("steam_id", &self.steam_id())
            .field("closed", &self.is_closed())
            .finish_non_exhaustive()
    }
}

impl State {
    /// One frame from Steam: an answer to a job or a logon, or something
    /// Steam says unasked.
    fn receive(&self, frame: &[u8]) {
        let packet = match Packet::decode(frame) {
            Ok(Some(packet)) => packet,
            Ok(None) => return,
            Err(e) => {
                self.log.line(&format!("skipped a message: {e}"));
                return;
            }
        };
        if packet.emsg == emsg::MULTI {
            match unpack_multi(&packet.body) {
                Ok(frames) => frames.iter().for_each(|f| self.receive(f)),
                Err(e) => self.log.line(&format!("skipped a message: {e}")),
            }
            return;
        }
        match packet.emsg {
            // An answer to a call: to whoever's waiting on its job, if
            // anyone still is.
            emsg::SERVICE_METHOD_RESPONSE => {
                let job = packet.header.jobid_target.filter(|&j| j != NO_JOB);
                let waiting = job.and_then(|job| self.pending.lock().unwrap().remove(&job));
                if let Some(waiting) = waiting {
                    let _ = waiting.send(packet);
                }
            }
            emsg::CLIENT_LOG_ON_RESPONSE => {
                if let Some(waiting) = self.logon.lock().unwrap().take() {
                    let _ = waiting.send(packet);
                }
            }
            emsg::CLIENT_PLAYING_SESSION_STATE => {
                if let Ok(s) =
                    proto::decode::<ClientPlayingSessionState>("playing state", &packet.body)
                {
                    let blocked = Blocked {
                        blocked: s.playing_blocked.unwrap_or_default(),
                        app_id: s.playing_app.filter(|&app| app != 0),
                    };
                    self.log.line(&format!("playing state: {blocked:?}"));
                    *self.blocked.lock().unwrap() = Some(blocked);
                    let _ = self.events.send(Event::PlayingBlocked(blocked));
                }
            }
            emsg::CLIENT_ITEM_ANNOUNCEMENTS => {
                if let Ok(a) =
                    proto::decode::<ClientItemAnnouncements>("item announcement", &packet.body)
                {
                    // The first after asking is the answer, when it comes
                    // promptly: no card drops within moments of playing. With
                    // nothing new, Steam may not answer at all, and a drop
                    // announced later is news, not what was there already.
                    let asked = self.asked_new_items.lock().unwrap().take();
                    let within = *self.answer_within.lock().unwrap();
                    let at_sign_on = asked.is_some_and(|at| at.elapsed() <= within);
                    // Whether Steam lists the items for card drops is still
                    // to be seen (research: market-and-session.md, section 6).
                    self.log.line(&format!(
                        "new items{}: {a:?}",
                        if at_sign_on { " at sign-on" } else { "" }
                    ));
                    let announced = Announcement {
                        count: a.count_new_items.unwrap_or_default(),
                        items: a.unseen_items.iter().filter_map(unseen).collect(),
                        at_sign_on,
                    };
                    if at_sign_on {
                        *self.new_at_sign_on.lock().unwrap() = Some(announced.clone());
                    }
                    let _ = self.events.send(Event::NewItems(announced));
                }
            }
            // Kept, as the playing state is: it comes as the session signs
            // on, before anyone could be listening for it.
            emsg::CLIENT_WALLET_INFO_UPDATE => {
                if let Ok(w) = proto::decode::<ClientWalletInfoUpdate>("wallet", &packet.body) {
                    let wallet = WalletInfo {
                        has_wallet: w.has_wallet.unwrap_or_default(),
                        currency: w.currency.unwrap_or_default(),
                    };
                    self.log.line(&format!("wallet: {wallet:?}"));
                    *self.wallet.lock().unwrap() = Some(wallet);
                }
            }
            emsg::CLIENT_LOGGED_OFF => {
                let eresult = proto::decode::<ClientLoggedOff>("sign-off", &packet.body)
                    .map_or(EResult::FAIL, |m| EResult::of(m.eresult));
                self.log.line(&format!("signed off by Steam: {eresult}"));
                *self.signed_on.lock().unwrap() = None;
                *self.logged_off.lock().unwrap() = Some(eresult);
                let _ = self.events.send(Event::LoggedOff(eresult));
            }
            // The rest of what Steam tells a client (friends, licences, …)
            // is nothing steamcards acts on.
            _ => {}
        }
    }

    /// The connection is gone: nothing waiting will be answered.
    fn close(&self) {
        self.pending.lock().unwrap().clear();
        self.logon.lock().unwrap().take();
        *self.signed_on.lock().unwrap() = None;
        self.log.line("connection closed");
        let _ = self.events.send(Event::Closed);
    }
}

/// An item as Steam lists it; `None` without an asset ID. Valve's messages
/// leave a field out, or send 0, when they don't say.
fn unseen(item: &proto::UnseenItem) -> Option<UnseenItem> {
    Some(UnseenItem {
        asset_id: item.asset_id.filter(|&id| id != 0)?,
        app_id: item.appid.unwrap_or_default(),
        context_id: item.context_id.unwrap_or_default(),
        gained_at: item.rtime32_gained.filter(|&at| at != 0),
        source_app_id: item.source_appid.filter(|&app| app != 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_item_steam_lists_says_what_it_says_and_no_more() {
        let listed = proto::UnseenItem {
            appid: Some(753),
            context_id: Some(6),
            asset_id: Some(31_002),
            amount: Some(1),
            rtime32_gained: Some(1_790_000_000),
            source_appid: Some(960_910),
        };
        assert_eq!(
            unseen(&listed),
            Some(UnseenItem {
                asset_id: 31_002,
                app_id: 753,
                context_id: 6,
                gained_at: Some(1_790_000_000),
                source_app_id: Some(960_910),
            })
        );

        let unsaid = proto::UnseenItem {
            rtime32_gained: Some(0),
            source_appid: None,
            ..listed.clone()
        };
        let item = unseen(&unsaid).unwrap();
        assert_eq!((item.gained_at, item.source_app_id), (None, None));

        let no_id = proto::UnseenItem {
            asset_id: Some(0),
            ..listed
        };
        assert_eq!(unseen(&no_id), None, "nothing to tell it by");
    }

    #[test]
    fn only_community_items_are_the_ones_described() {
        let card = UnseenItem {
            asset_id: 31_002,
            app_id: 753,
            context_id: 6,
            gained_at: None,
            source_app_id: Some(960_910),
        };
        let hat = UnseenItem {
            app_id: 440,
            context_id: 2,
            ..card
        };

        assert!(card.is_community_item());
        assert!(!hat.is_community_item(), "a Team Fortress 2 item");
    }
}
