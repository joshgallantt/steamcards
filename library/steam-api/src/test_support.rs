//! A stand-in for Steam, for tests here and in the data crates: a CM server
//! on this computer that signs in, signs on, plays, announces new items and
//! describes the items it holds as a test tells it to, and remembers what it
//! was sent. It speaks Steam's own messages over a real WebSocket, so the
//! code under test is the code that runs.

use std::{
    collections::{HashSet, VecDeque},
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
    cm::UnseenItem,
    inventory::{COMMUNITY_CONTEXT, STEAM_APP},
    packet::{Packet, pack_multi},
    proto::{
        self, Asset, BeginAuthSessionViaQrResponse, ClientChangeStatus, ClientGamesPlayed,
        ClientItemAnnouncements, ClientLoggedOff, ClientLogon, ClientLogonResponse,
        ClientPlayingSessionState, GenerateAccessTokenResponse, GetInventoryItemsRequest,
        GetInventoryItemsResponse, Header, ItemDescription, ItemTag, PollAuthSessionStatusResponse,
        RevokeTokenRequest, RevokeTokenResponse, emsg, method,
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

/// An item in the stand-in account's inventory, among its community items,
/// as Steam describes it. Items with one market hash name are one class,
/// and share a description.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldItem {
    pub asset_id: u64,
    /// Its name on the market, as Steam gives it: "Chell (Foil)".
    pub market_name: String,
    pub market_hash_name: String,
    /// The app the publisher's share of a sale goes to: its game.
    pub market_fee_app: u32,
    /// Its tags, as `(category, internal name)`.
    pub tags: Vec<(String, String)>,
    pub marketable: bool,
    pub tradable: bool,
    /// Steam hasn't caught up with it yet: the first ask about it finds it
    /// missing, and the next finds it.
    pub late: bool,
}

impl HeldItem {
    /// A card from `app_id`'s set, named as the market names it: "Chell", or
    /// "Intro (Trading Card)" where the name clashes with another item's.
    pub fn card(asset_id: u64, app_id: u32, market_name: &str) -> Self {
        Self::tagged(
            asset_id,
            app_id,
            market_name,
            &[
                ("item_class", "item_class_2"),
                ("cardborder", "cardborder_0"),
            ],
        )
    }

    /// A foil card: "Chell (Foil)", or "Intro (Foil Trading Card)".
    pub fn foil(asset_id: u64, app_id: u32, market_name: &str) -> Self {
        Self::tagged(
            asset_id,
            app_id,
            market_name,
            &[
                ("item_class", "item_class_2"),
                ("cardborder", "cardborder_1"),
            ],
        )
    }

    /// Anything else from `app_id`, of Steam's item class `class`: 3 is a
    /// profile background, 4 an emoticon, 5 a booster pack.
    pub fn other(asset_id: u64, app_id: u32, class: u32, market_name: &str) -> Self {
        let class = format!("item_class_{class}");
        Self::tagged(asset_id, app_id, market_name, &[("item_class", &class)])
    }

    /// One Steam hasn't caught up with yet.
    pub fn late(self) -> Self {
        Self { late: true, ..self }
    }

    fn tagged(asset_id: u64, app_id: u32, market_name: &str, tags: &[(&str, &str)]) -> Self {
        let game = format!("app_{app_id}");
        Self {
            asset_id,
            market_name: market_name.to_owned(),
            market_hash_name: format!("{app_id}-{market_name}"),
            market_fee_app: app_id,
            tags: [("Game", game.as_str())]
                .iter()
                .chain(tags)
                .map(|&(category, name)| (category.to_owned(), name.to_owned()))
                .collect(),
            marketable: true,
            tradable: true,
            late: false,
        }
    }
}

/// A trading card from `game` among the account's community items, just
/// arrived, as Steam lists a new item.
pub fn unseen_card(asset_id: u64, game: u32) -> UnseenItem {
    UnseenItem {
        asset_id,
        app_id: STEAM_APP,
        context_id: COMMUNITY_CONTEXT,
        gained_at: u32::try_from(now()).ok(),
        source_app_id: Some(game),
    }
}

/// An ask to describe items, as the stand-in heard it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryAsk {
    pub steam_id: u64,
    pub app_id: u32,
    pub context_id: u64,
    /// Whether it asked for what the items are, not just their IDs.
    pub descriptions: bool,
    pub language: String,
    pub asset_ids: Vec<u64>,
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
    inventory: Vec<HeldItem>,
    /// Asset IDs asked about so far.
    asked_about: HashSet<u64>,
    inventory_asks: Vec<InventoryAsk>,
    /// New items not seen yet: Steam lists them until the inventory is
    /// viewed.
    unseen: Vec<UnseenItem>,
    /// How often a session asked what's new.
    announcement_asks: usize,
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
            inventory: Vec::new(),
            asked_about: HashSet::new(),
            inventory_asks: Vec::new(),
            unseen: Vec::new(),
            announcement_asks: 0,
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

    /// The account's community items are these, in place of any before.
    pub fn hold(&self, items: Vec<HeldItem>) {
        self.state().inventory = items;
    }

    /// Every ask to describe items, in order.
    pub fn inventory_asks(&self) -> Vec<InventoryAsk> {
        self.state().inventory_asks.clone()
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

    /// Steam says how many new items there are, and not which.
    pub fn new_items(&self, count: u32) {
        self.push(&Packet::encode(
            emsg::CLIENT_ITEM_ANNOUNCEMENTS,
            &Header::default(),
            &ClientItemAnnouncements {
                count_new_items: Some(count),
                unseen_items: Vec::new(),
            },
        ));
    }

    /// These items arrive, and Steam announces them: it lists every item
    /// not seen yet, these among them.
    pub fn announce(&self, items: Vec<UnseenItem>) {
        let frame = {
            let mut s = self.state();
            s.unseen.extend(items);
            announcement(&s.unseen)
        };
        self.push(&frame);
    }

    /// These items arrived before any session signed on, and haven't been
    /// seen: Steam lists them when asked.
    pub fn already_unseen(&self, items: Vec<UnseenItem>) {
        self.state().unseen.extend(items);
    }

    /// The inventory is viewed: nothing is new any more, and Steam says so.
    pub fn inventory_viewed(&self) {
        self.state().unseen.clear();
        self.push(&announcement(&[]));
    }

    /// How often a session asked Steam what's new.
    pub fn announcement_asks(&self) -> usize {
        self.state().announcement_asks
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
            emsg::CLIENT_REQUEST_ITEM_ANNOUNCEMENTS => {
                self.announcement_asks += 1;
                vec![announcement(&self.unseen)]
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
            method::GET_INVENTORY_ITEMS if signed_on => {
                let req = GetInventoryItemsRequest::decode(body).unwrap_or_default();
                let ask = InventoryAsk {
                    steam_id: req.steamid.unwrap_or_default(),
                    app_id: req.appid.unwrap_or_default(),
                    context_id: req.contextid.unwrap_or_default(),
                    descriptions: req.get_descriptions.unwrap_or_default(),
                    language: req.language.unwrap_or_default(),
                    asset_ids: req.filters.map(|f| f.assetids).unwrap_or_default(),
                };
                let items = self.items(&ask);
                self.inventory_asks.push(ask);
                (EResult::OK, items.encode_to_vec())
            }
            method::GENERATE_ACCESS_TOKEN | method::REVOKE_TOKEN | method::GET_INVENTORY_ITEMS => {
                (EResult::ACCESS_DENIED, Vec::new())
            }
            _ => (EResult::FAIL, Vec::new()),
        }
    }

    /// Steam's answer to `ask`: each item it names that the stand-in account
    /// holds among its community items, unless Steam hasn't caught up with it
    /// yet; the rest are missing. An ask naming no items is for them all.
    fn items(&mut self, ask: &InventoryAsk) -> GetInventoryItemsResponse {
        let ours =
            (ask.steam_id, ask.app_id, ask.context_id) == (STEAM_ID, STEAM_APP, COMMUNITY_CONTEXT);
        let named = if ask.asset_ids.is_empty() {
            self.inventory.iter().map(|i| i.asset_id).collect()
        } else {
            ask.asset_ids.clone()
        };
        let mut answer = GetInventoryItemsResponse::default();
        for id in named {
            let first_ask = self.asked_about.insert(id);
            let held = self
                .inventory
                .iter()
                .find(|i| ours && i.asset_id == id && !(i.late && first_ask));
            let Some(item) = held else {
                answer.missing_assets.push(Asset {
                    assetid: Some(id),
                    ..Default::default()
                });
                continue;
            };
            let class = self.class_of(item);
            answer.assets.push(Asset {
                assetid: Some(id),
                classid: Some(class),
                instanceid: Some(0),
            });
            let described = answer.descriptions.iter().any(|d| d.classid == Some(class));
            if ask.descriptions && !described {
                answer.descriptions.push(description(item, class));
            }
        }
        answer
    }

    /// The class of items that share `item`'s market hash name.
    fn class_of(&self, item: &HeldItem) -> u64 {
        let first = self
            .inventory
            .iter()
            .position(|i| i.market_hash_name == item.market_hash_name)
            .unwrap_or_default();
        1_000 + first as u64
    }

    /// A new refresh token, unlike any before it.
    fn issue(&mut self) -> String {
        self.issued += 1;
        token::fake(STEAM_ID, now() + 200 * 24 * 60 * 60 + self.issued)
    }
}

/// What Steam says items of `item`'s class are.
fn description(item: &HeldItem, class: u64) -> ItemDescription {
    ItemDescription {
        classid: Some(class),
        instanceid: Some(0),
        tradable: Some(item.tradable),
        name: Some(item.market_name.clone()),
        market_name: Some(item.market_name.clone()),
        market_hash_name: Some(item.market_hash_name.clone()),
        marketable: Some(item.marketable),
        tags: item
            .tags
            .iter()
            .map(|(category, name)| ItemTag {
                category: Some(category.clone()),
                internal_name: Some(name.clone()),
            })
            .collect(),
        market_fee_app: i32::try_from(item.market_fee_app).ok(),
    }
}

/// `ClientItemAnnouncements`, counting and listing `unseen`.
fn announcement(unseen: &[UnseenItem]) -> Vec<u8> {
    Packet::encode(
        emsg::CLIENT_ITEM_ANNOUNCEMENTS,
        &Header::default(),
        &ClientItemAnnouncements {
            count_new_items: u32::try_from(unseen.len()).ok(),
            unseen_items: unseen
                .iter()
                .map(|item| proto::UnseenItem {
                    appid: Some(item.app_id),
                    context_id: Some(item.context_id),
                    asset_id: Some(item.asset_id),
                    amount: Some(1),
                    rtime32_gained: item.gained_at,
                    source_appid: item.source_app_id,
                })
                .collect(),
        },
    )
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
