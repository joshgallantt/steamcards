//! A stand-in for Steam, for tests here and in the data crates: a CM server
//! on this computer that signs in, signs on and plays as a test tells it to,
//! and remembers what it was sent. It speaks Steam's own messages over a real
//! WebSocket, so the code under test is the code that runs.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use futures::{SinkExt, StreamExt};
use prost::Message;
use tokio::{net::TcpListener, sync::mpsc};
use tokio_tungstenite::tungstenite::Message as Frame;
use tokio_util::sync::{CancellationToken, DropGuard};

use crate::{
    EResult, Endpoints,
    packet::{Packet, pack_multi},
    proto::{
        BeginAuthSessionViaQrResponse, ClientChangeStatus, ClientGamesPlayed,
        ClientItemAnnouncements, ClientLoggedOff, ClientLogon, ClientLogonResponse,
        ClientPlayingSessionState, GenerateAccessTokenResponse, Header,
        PollAuthSessionStatusResponse, RevokeTokenRequest, RevokeTokenResponse, emsg, method,
    },
    token,
};

/// The account the stand-in signs in.
pub const STEAM_ID: u64 = 76_561_197_960_287_930;
pub const ACCOUNT: &str = "cardfarmer";

/// A token shaped like Steam's: `steam_id`'s, expiring at `expires_at`
/// (seconds since the epoch), with no real signature.
pub fn token(steam_id: u64, expires_at: i64) -> String {
    token::fake(steam_id, expires_at)
}

/// How one QR sign-in goes: on which poll (counting from 1) a new code is
/// sent, the code is scanned, the sign-in is approved, or it ends unapproved.
#[derive(Debug, Clone, Default)]
pub struct QrScript {
    pub new_code_at: Option<usize>,
    pub scanned_at: Option<usize>,
    pub approved_at: Option<usize>,
    pub ended_at: Option<usize>,
}

struct State {
    /// One script per QR sign-in begun; the last one repeats.
    qr: VecDeque<QrScript>,
    qr_begun: usize,
    polls: usize,
    signing_in: Option<QrScript>,
    logon: EResult,
    blocked_at_logon: Option<u32>,
    renews: bool,
    issued: i64,
    logons: Vec<String>,
    games: Vec<Vec<u32>>,
    statuses: Vec<u32>,
    revoked: Vec<String>,
    log_offs: usize,
    clients: Vec<mpsc::UnboundedSender<Vec<u8>>>,
}

pub struct FakeSteam {
    url: String,
    state: Arc<Mutex<State>>,
    _stop: DropGuard,
}

impl FakeSteam {
    /// Starts listening on a local port. Until told otherwise, a QR code is
    /// approved on the first poll, and signing on works.
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("a local port");
        let url = format!(
            "ws://{}/cmsocket/",
            listener.local_addr().expect("its address")
        );
        let state = Arc::new(Mutex::new(State {
            qr: VecDeque::from([QrScript {
                approved_at: Some(1),
                ..Default::default()
            }]),
            qr_begun: 0,
            polls: 0,
            signing_in: None,
            logon: EResult::OK,
            blocked_at_logon: None,
            renews: false,
            issued: 0,
            logons: Vec::new(),
            games: Vec::new(),
            statuses: Vec::new(),
            revoked: Vec::new(),
            log_offs: 0,
            clients: Vec::new(),
        }));
        let stop = CancellationToken::new();
        let (shared, stopping) = (state.clone(), stop.clone());
        tokio::spawn(async move {
            loop {
                let accepted = tokio::select! {
                    _ = stopping.cancelled() => break,
                    accepted = listener.accept() => accepted,
                };
                if let Ok((tcp, _)) = accepted {
                    tokio::spawn(serve(tcp, shared.clone(), stopping.clone()));
                }
            }
        });
        Self {
            url,
            state,
            _stop: stop.drop_guard(),
        }
    }

    /// Endpoints with this as the CM server. Nothing listens at the Web API
    /// or community addresses: a test that needs them sets its own.
    pub fn endpoints(&self) -> Endpoints {
        Endpoints {
            api: "http://127.0.0.1:9".into(),
            community: "http://127.0.0.1:9".into(),
            cm: Some(self.url.clone()),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    /// How the next QR sign-ins go, one script each; the last repeats.
    pub fn qr_goes(&self, scripts: Vec<QrScript>) {
        self.state().qr = scripts.into();
    }

    /// How many QR sign-ins have begun.
    pub fn qr_codes_begun(&self) -> usize {
        self.state().qr_begun
    }

    /// Signing on is refused with `e` from now on.
    pub fn refuse_logon(&self, e: EResult) {
        self.state().logon = e;
    }

    /// Another device is playing `app_id` when a session signs on.
    pub fn busy_elsewhere(&self, app_id: u32) {
        self.state().blocked_at_logon = Some(app_id);
    }

    /// Steam renews the refresh token whenever it makes a site token.
    pub fn renew_refresh_tokens(&self) {
        self.state().renews = true;
    }

    /// The refresh tokens sessions signed on with, in order.
    pub fn logons(&self) -> Vec<String> {
        self.state().logons.clone()
    }

    /// Every list of games a session said it was playing, in order.
    pub fn games_played(&self) -> Vec<Vec<u32>> {
        self.state().games.clone()
    }

    /// Every persona state a session set: 0 offline, 1 online.
    pub fn statuses(&self) -> Vec<u32> {
        self.state().statuses.clone()
    }

    /// Refresh tokens a session asked to be revoked.
    pub fn revoked(&self) -> Vec<String> {
        self.state().revoked.clone()
    }

    pub fn log_offs(&self) -> usize {
        self.state().log_offs
    }

    /// Another device starts (or stops) playing.
    pub fn block(&self, blocked: bool, app_id: u32) {
        self.push(&Packet::encode(
            emsg::CLIENT_PLAYING_SESSION_STATE,
            &Header::default(),
            &ClientPlayingSessionState {
                playing_blocked: Some(blocked),
                playing_app: Some(app_id),
            },
        ));
    }

    /// New items arrive in the inventory.
    pub fn new_items(&self, count: u32) {
        self.push(&Packet::encode(
            emsg::CLIENT_ITEM_ANNOUNCEMENTS,
            &Header::default(),
            &ClientItemAnnouncements {
                count_new_items: Some(count),
            },
        ));
    }

    /// Steam signs every session off, saying why.
    pub fn sign_off(&self, why: EResult) {
        self.push(&signed_off(why));
    }

    /// The connections drop.
    pub fn hang_up(&self) {
        self.push(&[]);
    }

    fn push(&self, frame: &[u8]) {
        self.state()
            .clients
            .retain(|c| c.send(frame.to_vec()).is_ok());
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap()
    }
}

/// One client's connection: answers what it sends, and passes on what a
/// test pushes. An empty frame hangs up.
async fn serve(tcp: tokio::net::TcpStream, state: Arc<Mutex<State>>, stop: CancellationToken) {
    let Ok(ws) = tokio_tungstenite::accept_async(tcp).await else {
        return;
    };
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    state.lock().unwrap().clients.push(tx.clone());
    loop {
        tokio::select! {
            _ = stop.cancelled() => break,
            out = rx.recv() => match out {
                Some(frame) if !frame.is_empty() => {
                    if sink.send(Frame::Binary(frame)).await.is_err() {
                        break;
                    }
                }
                _ => break,
            },
            received = stream.next() => match received {
                Some(Ok(Frame::Binary(frame))) => {
                    for answer in state.lock().unwrap().answer(&frame) {
                        let _ = tx.send(answer);
                    }
                }
                Some(Ok(_)) => {}
                _ => break,
            },
        }
    }
    let _ = sink.close().await;
}

impl State {
    /// What Steam would send back for `frame`.
    fn answer(&mut self, frame: &[u8]) -> Vec<Vec<u8>> {
        let Ok(Some(packet)) = Packet::decode(frame) else {
            return Vec::new();
        };
        match packet.emsg {
            emsg::SERVICE_METHOD_CALL_FROM_CLIENT
            | emsg::SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED => {
                let signed_on = packet.emsg == emsg::SERVICE_METHOD_CALL_FROM_CLIENT;
                let name = packet.header.target_job_name.clone().unwrap_or_default();
                let (eresult, body) = self.call(&name, &packet.body, signed_on);
                let header = Header {
                    jobid_target: packet.header.jobid_source,
                    eresult: Some(eresult.0),
                    ..Default::default()
                };
                vec![Packet::encode_bytes(
                    emsg::SERVICE_METHOD_RESPONSE,
                    &header,
                    &body,
                )]
            }
            emsg::CLIENT_LOGON => {
                let logon = ClientLogon::decode(&packet.body[..]).unwrap_or_default();
                self.logons.push(logon.access_token.unwrap_or_default());
                let header = Header {
                    steamid: Some(STEAM_ID),
                    client_sessionid: Some(77),
                    ..Default::default()
                };
                let mut frames = vec![Packet::encode(
                    emsg::CLIENT_LOG_ON_RESPONSE,
                    &header,
                    &ClientLogonResponse {
                        eresult: Some(self.logon.0),
                        legacy_out_of_game_heartbeat_seconds: Some(9),
                        heartbeat_seconds: Some(9),
                    },
                )];
                if let Some(app) = self.blocked_at_logon.filter(|_| self.logon.is_ok()) {
                    frames.push(Packet::encode(
                        emsg::CLIENT_PLAYING_SESSION_STATE,
                        &Header::default(),
                        &ClientPlayingSessionState {
                            playing_blocked: Some(true),
                            playing_app: Some(app),
                        },
                    ));
                }
                // Steam packs what it sends at sign-on into one Multi.
                vec![Packet::encode(
                    emsg::MULTI,
                    &Header::default(),
                    &pack_multi(&frames),
                )]
            }
            emsg::CLIENT_GAMES_PLAYED_WITH_DATA_BLOB => {
                let played = ClientGamesPlayed::decode(&packet.body[..]).unwrap_or_default();
                self.games.push(
                    played
                        .games_played
                        .iter()
                        .filter_map(|g| g.game_id.map(|id| id as u32))
                        .collect(),
                );
                Vec::new()
            }
            emsg::CLIENT_CHANGE_STATUS => {
                let status = ClientChangeStatus::decode(&packet.body[..]).unwrap_or_default();
                self.statuses.push(status.persona_state.unwrap_or_default());
                Vec::new()
            }
            emsg::CLIENT_LOG_OFF => {
                self.log_offs += 1;
                vec![signed_off(EResult::OK), Vec::new()]
            }
            _ => Vec::new(),
        }
    }

    /// A service method's answer: its `EResult` and body.
    fn call(&mut self, name: &str, body: &[u8], signed_on: bool) -> (EResult, Vec<u8>) {
        match name {
            method::BEGIN_QR => {
                let script = if self.qr.len() > 1 {
                    self.qr.pop_front().unwrap_or_default()
                } else {
                    self.qr.front().cloned().unwrap_or_default()
                };
                self.qr_begun += 1;
                self.polls = 0;
                self.signing_in = Some(script);
                let n = self.qr_begun as u64;
                let begun = BeginAuthSessionViaQrResponse {
                    client_id: Some(1000 + n),
                    challenge_url: Some(format!("https://s.team/q/1/{n}")),
                    request_id: Some(b"request".to_vec()),
                    interval: Some(0.02),
                };
                (EResult::OK, begun.encode_to_vec())
            }
            method::POLL => {
                self.polls += 1;
                let n = self.polls;
                let Some(q) = self.signing_in.clone() else {
                    return (EResult::FILE_NOT_FOUND, Vec::new());
                };
                if q.ended_at == Some(n) {
                    self.signing_in = None;
                    return (EResult::FILE_NOT_FOUND, Vec::new());
                }
                if q.approved_at.is_some_and(|at| n >= at) {
                    self.signing_in = None;
                    let approved = PollAuthSessionStatusResponse {
                        refresh_token: Some(self.issue()),
                        access_token: Some(token::fake(STEAM_ID, now() + 24 * 60 * 60)),
                        account_name: Some(ACCOUNT.into()),
                        ..Default::default()
                    };
                    return (EResult::OK, approved.encode_to_vec());
                }
                let waiting = PollAuthSessionStatusResponse {
                    new_challenge_url: (q.new_code_at == Some(n))
                        .then(|| format!("https://s.team/q/1/{}-{n}", self.qr_begun)),
                    had_remote_interaction: q.scanned_at.map(|at| n >= at),
                    ..Default::default()
                };
                (EResult::OK, waiting.encode_to_vec())
            }
            method::GENERATE_ACCESS_TOKEN if signed_on => {
                let made = GenerateAccessTokenResponse {
                    access_token: Some(token::fake(STEAM_ID, now() + 24 * 60 * 60)),
                    refresh_token: self.renews.then(|| self.issue()),
                };
                (EResult::OK, made.encode_to_vec())
            }
            method::REVOKE_TOKEN if signed_on => {
                let req = RevokeTokenRequest::decode(body).unwrap_or_default();
                self.revoked.push(req.token.unwrap_or_default());
                (EResult::OK, RevokeTokenResponse {}.encode_to_vec())
            }
            method::GENERATE_ACCESS_TOKEN | method::REVOKE_TOKEN => {
                (EResult::ACCESS_DENIED, Vec::new())
            }
            _ => (EResult::FAIL, Vec::new()),
        }
    }

    /// A new refresh token, unlike any before it.
    fn issue(&mut self) -> String {
        self.issued += 1;
        token::fake(STEAM_ID, now() + 200 * 24 * 60 * 60 + self.issued)
    }
}

/// `ClientLoggedOff`, saying why.
fn signed_off(why: EResult) -> Vec<u8> {
    Packet::encode(
        emsg::CLIENT_LOGGED_OFF,
        &Header::default(),
        &ClientLoggedOff {
            eresult: Some(why.0),
        },
    )
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}
