//! Steam's messages, as far as steamcards sends and reads them. Names, field
//! numbers and types follow Valve's own .proto files, as
//! [SteamDatabase/Protobufs](https://github.com/SteamDatabase/Protobufs)
//! publishes them: a message here decodes what Steam sends, and what it
//! encodes, Steam reads. Fields steamcards never sets or reads are left out;
//! protobuf skips what it doesn't know.
//!
//! Enum fields are written as the `int32`s they are on the wire.

use prost::Message;

/// Message numbers (`EMsg`), from `enums_clientserver.proto`.
pub(crate) mod emsg {
    pub(crate) const MULTI: u32 = 1;
    pub(crate) const SERVICE_METHOD_RESPONSE: u32 = 147;
    pub(crate) const SERVICE_METHOD_CALL_FROM_CLIENT: u32 = 151;
    pub(crate) const CLIENT_HEART_BEAT: u32 = 703;
    pub(crate) const CLIENT_LOG_OFF: u32 = 706;
    pub(crate) const CLIENT_CHANGE_STATUS: u32 = 716;
    pub(crate) const CLIENT_LOG_ON_RESPONSE: u32 = 751;
    pub(crate) const CLIENT_LOGGED_OFF: u32 = 757;
    pub(crate) const CLIENT_GAMES_PLAYED_WITH_DATA_BLOB: u32 = 5410;
    pub(crate) const CLIENT_LOGON: u32 = 5514;
    pub(crate) const CLIENT_WALLET_INFO_UPDATE: u32 = 5528;
    pub(crate) const CLIENT_ITEM_ANNOUNCEMENTS: u32 = 5576;
    pub(crate) const CLIENT_PLAYING_SESSION_STATE: u32 = 9600;
    pub(crate) const SERVICE_METHOD_CALL_FROM_CLIENT_NON_AUTHED: u32 = 9804;
    pub(crate) const CLIENT_HELLO: u32 = 9805;
}

/// The protocol version the Steam client speaks, as SteamKit and steam-vent
/// send it.
pub(crate) const PROTOCOL_VERSION: u32 = 65580;

/// `CMsgProtoBufHeader`: who a message is from, and which request it answers.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct Header {
    #[prost(fixed64, optional, tag = "1")]
    pub(crate) steamid: Option<u64>,
    #[prost(int32, optional, tag = "2")]
    pub(crate) client_sessionid: Option<i32>,
    #[prost(fixed64, optional, tag = "10")]
    pub(crate) jobid_source: Option<u64>,
    #[prost(fixed64, optional, tag = "11")]
    pub(crate) jobid_target: Option<u64>,
    #[prost(string, optional, tag = "12")]
    pub(crate) target_job_name: Option<String>,
    /// `EResult`. Absent means 2 (fail), as the .proto's default says.
    #[prost(int32, optional, tag = "13")]
    pub(crate) eresult: Option<i32>,
    #[prost(string, optional, tag = "14")]
    pub(crate) error_message: Option<String>,
}

/// `CMsgMulti`: several messages in one, gzipped when `size_unzipped` is set.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct Multi {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) size_unzipped: Option<u32>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub(crate) message_body: Option<Vec<u8>>,
}

/// `CMsgClientHello`: the first thing a client says.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientHello {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) protocol_version: Option<u32>,
}

/// `CMsgClientHeartBeat`: "still here", every few seconds once signed on.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientHeartBeat {
    #[prost(bool, optional, tag = "1")]
    pub(crate) send_reply: Option<bool>,
}

/// `CMsgIPAddress`, with its `oneof` written as the one field steamcards
/// uses: an IPv4 address is the same bytes on the wire either way.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct IpAddress {
    #[prost(fixed32, optional, tag = "1")]
    pub(crate) v4: Option<u32>,
}

/// `CMsgClientLogon`, as the Steam client signs on with a refresh token.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientLogon {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) protocol_version: Option<u32>,
    #[prost(uint32, optional, tag = "5")]
    pub(crate) client_package_version: Option<u32>,
    #[prost(string, optional, tag = "6")]
    pub(crate) client_language: Option<String>,
    #[prost(uint32, optional, tag = "7")]
    pub(crate) client_os_type: Option<u32>,
    #[prost(bool, optional, tag = "8")]
    pub(crate) should_remember_password: Option<bool>,
    /// Where SteamKit puts the login ID, which tells this computer's
    /// sessions apart.
    #[prost(message, optional, tag = "11")]
    pub(crate) obfuscated_private_ip: Option<IpAddress>,
    #[prost(uint32, optional, tag = "33")]
    pub(crate) chat_mode: Option<u32>,
    #[prost(string, optional, tag = "50")]
    pub(crate) account_name: Option<String>,
    #[prost(string, optional, tag = "96")]
    pub(crate) machine_name: Option<String>,
    /// The refresh token, whatever the field is called.
    #[prost(string, optional, tag = "108")]
    pub(crate) access_token: Option<String>,
}

/// `CMsgClientLogonResponse`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientLogonResponse {
    #[prost(int32, optional, tag = "1")]
    pub(crate) eresult: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    pub(crate) legacy_out_of_game_heartbeat_seconds: Option<i32>,
    #[prost(int32, optional, tag = "3")]
    pub(crate) heartbeat_seconds: Option<i32>,
}

/// `CMsgClientLogOff`: signing off, from the client.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientLogOff {}

/// `CMsgClientLoggedOff`: Steam signed this session off.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientLoggedOff {
    #[prost(int32, optional, tag = "1")]
    pub(crate) eresult: Option<i32>,
}

/// `CMsgClientGamesPlayed`: what this session is playing. Sent as
/// `ClientGamesPlayedWithDataBlob`, as ASF and node-steam-user send it.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientGamesPlayed {
    #[prost(message, repeated, tag = "1")]
    pub(crate) games_played: Vec<GamePlayed>,
    #[prost(uint32, optional, tag = "2")]
    pub(crate) client_os_type: Option<u32>,
}

/// `CMsgClientGamesPlayed.GamePlayed`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct GamePlayed {
    /// The app ID, for a plain Steam game.
    #[prost(fixed64, optional, tag = "2")]
    pub(crate) game_id: Option<u64>,
}

/// `EPersonaState`: offline, which a session is until it says otherwise, and
/// online.
pub(crate) const PERSONA_OFFLINE: u32 = 0;
pub(crate) const PERSONA_ONLINE: u32 = 1;

/// `CMsgClientChangeStatus`: how this session shows to friends.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientChangeStatus {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) persona_state: Option<u32>,
}

/// `CMsgClientPlayingSessionState`: whether another session is playing,
/// which stops this one from playing.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientPlayingSessionState {
    #[prost(bool, optional, tag = "2")]
    pub(crate) playing_blocked: Option<bool>,
    #[prost(uint32, optional, tag = "3")]
    pub(crate) playing_app: Option<u32>,
}

/// `CMsgClientItemAnnouncements`: new items in the inventory, such as a card
/// that just dropped.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientItemAnnouncements {
    #[prost(uint32, optional, tag = "1")]
    pub(crate) count_new_items: Option<u32>,
}

/// `CMsgClientWalletInfoUpdate`: the account's Steam wallet, which Steam
/// tells a session as it signs on. Its currency is the one the market
/// answers in, signed in.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ClientWalletInfoUpdate {
    #[prost(bool, optional, tag = "1")]
    pub(crate) has_wallet: Option<bool>,
    /// `ECurrency`.
    #[prost(int32, optional, tag = "3")]
    pub(crate) currency: Option<i32>,
}

/// `EAuthTokenPlatformType_SteamClient`: a sign-in for the Steam client,
/// whose token signs on to a CM server.
pub(crate) const PLATFORM_STEAM_CLIENT: i32 = 1;

/// `CAuthentication_DeviceDetails`: what the Steam app shows it's approving.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct DeviceDetails {
    #[prost(string, optional, tag = "1")]
    pub(crate) device_friendly_name: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub(crate) platform_type: Option<i32>,
    #[prost(int32, optional, tag = "3")]
    pub(crate) os_type: Option<i32>,
}

/// `CAuthentication_BeginAuthSessionViaQR_Request`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct BeginAuthSessionViaQrRequest {
    #[prost(string, optional, tag = "1")]
    pub(crate) device_friendly_name: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub(crate) platform_type: Option<i32>,
    #[prost(message, optional, tag = "3")]
    pub(crate) device_details: Option<DeviceDetails>,
    #[prost(string, optional, tag = "4")]
    pub(crate) website_id: Option<String>,
}

/// `CAuthentication_BeginAuthSessionViaQR_Response`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct BeginAuthSessionViaQrResponse {
    #[prost(uint64, optional, tag = "1")]
    pub(crate) client_id: Option<u64>,
    #[prost(string, optional, tag = "2")]
    pub(crate) challenge_url: Option<String>,
    #[prost(bytes = "vec", optional, tag = "3")]
    pub(crate) request_id: Option<Vec<u8>>,
    /// Seconds between polls.
    #[prost(float, optional, tag = "4")]
    pub(crate) interval: Option<f32>,
}

/// `CAuthentication_PollAuthSessionStatus_Request`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct PollAuthSessionStatusRequest {
    #[prost(uint64, optional, tag = "1")]
    pub(crate) client_id: Option<u64>,
    #[prost(bytes = "vec", optional, tag = "2")]
    pub(crate) request_id: Option<Vec<u8>>,
}

/// `CAuthentication_PollAuthSessionStatus_Response`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct PollAuthSessionStatusResponse {
    #[prost(uint64, optional, tag = "1")]
    pub(crate) new_client_id: Option<u64>,
    #[prost(string, optional, tag = "2")]
    pub(crate) new_challenge_url: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub(crate) refresh_token: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub(crate) access_token: Option<String>,
    /// The QR code has been scanned: the Steam app is asking to approve.
    #[prost(bool, optional, tag = "5")]
    pub(crate) had_remote_interaction: Option<bool>,
    #[prost(string, optional, tag = "6")]
    pub(crate) account_name: Option<String>,
}

/// `ETokenRenewalType_Allow`: Steam may hand back a new refresh token too.
pub(crate) const RENEWAL_ALLOW: i32 = 1;

/// `CAuthentication_AccessToken_GenerateForApp_Request`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct GenerateAccessTokenRequest {
    #[prost(string, optional, tag = "1")]
    pub(crate) refresh_token: Option<String>,
    #[prost(fixed64, optional, tag = "2")]
    pub(crate) steamid: Option<u64>,
    #[prost(int32, optional, tag = "3")]
    pub(crate) renewal_type: Option<i32>,
}

/// `CAuthentication_AccessToken_GenerateForApp_Response`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct GenerateAccessTokenResponse {
    #[prost(string, optional, tag = "1")]
    pub(crate) access_token: Option<String>,
    #[prost(string, optional, tag = "2")]
    pub(crate) refresh_token: Option<String>,
}

/// `EAuthTokenRevokeLogout`: what signing out of the Steam client does.
pub(crate) const REVOKE_LOGOUT: i32 = 0;

/// `CAuthentication_Token_Revoke_Request`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct RevokeTokenRequest {
    #[prost(string, optional, tag = "1")]
    pub(crate) token: Option<String>,
    #[prost(int32, optional, tag = "2")]
    pub(crate) revoke_action: Option<i32>,
}

/// `CAuthentication_Token_Revoke_Response`: nothing in it.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct RevokeTokenResponse {}

/// `CEcon_GetInventoryItemsWithDescriptions_Request`: items in an
/// inventory, and what they are.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct GetInventoryItemsRequest {
    #[prost(fixed64, optional, tag = "1")]
    pub(crate) steamid: Option<u64>,
    #[prost(uint32, optional, tag = "2")]
    pub(crate) appid: Option<u32>,
    #[prost(uint64, optional, tag = "3")]
    pub(crate) contextid: Option<u64>,
    #[prost(bool, optional, tag = "4")]
    pub(crate) get_descriptions: Option<bool>,
    #[prost(string, optional, tag = "5")]
    pub(crate) language: Option<String>,
    #[prost(message, optional, tag = "6")]
    pub(crate) filters: Option<FilterOptions>,
}

/// `CEcon_GetInventoryItemsWithDescriptions_Request.FilterOptions`: which
/// items. With no asset IDs, every one.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct FilterOptions {
    /// Valve's files are proto2, where a repeated number isn't packed.
    #[prost(uint64, repeated, packed = "false", tag = "1")]
    pub(crate) assetids: Vec<u64>,
}

/// `CEcon_GetInventoryItemsWithDescriptions_Response`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct GetInventoryItemsResponse {
    #[prost(message, repeated, tag = "1")]
    pub(crate) assets: Vec<Asset>,
    /// One per class and instance of item, however many assets share it.
    #[prost(message, repeated, tag = "2")]
    pub(crate) descriptions: Vec<ItemDescription>,
    /// Items asked for that Steam doesn't have.
    #[prost(message, repeated, tag = "3")]
    pub(crate) missing_assets: Vec<Asset>,
}

/// `CEcon_Asset`: one item in an inventory.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct Asset {
    #[prost(uint64, optional, tag = "3")]
    pub(crate) assetid: Option<u64>,
    #[prost(uint64, optional, tag = "4")]
    pub(crate) classid: Option<u64>,
    #[prost(uint64, optional, tag = "5")]
    pub(crate) instanceid: Option<u64>,
}

/// `CEconItem_Description`: what items of one class and instance are.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ItemDescription {
    #[prost(uint64, optional, tag = "2")]
    pub(crate) classid: Option<u64>,
    #[prost(uint64, optional, tag = "3")]
    pub(crate) instanceid: Option<u64>,
    #[prost(bool, optional, tag = "9")]
    pub(crate) tradable: Option<bool>,
    #[prost(string, optional, tag = "14")]
    pub(crate) name: Option<String>,
    #[prost(string, optional, tag = "17")]
    pub(crate) market_name: Option<String>,
    #[prost(string, optional, tag = "18")]
    pub(crate) market_hash_name: Option<String>,
    #[prost(bool, optional, tag = "25")]
    pub(crate) marketable: Option<bool>,
    #[prost(message, repeated, tag = "26")]
    pub(crate) tags: Vec<ItemTag>,
    /// The app the publisher's share of a sale goes to: the game.
    #[prost(int32, optional, tag = "28")]
    pub(crate) market_fee_app: Option<i32>,
}

/// `CEconItem_Tag`: one of an item's tags, such as `item_class_2` in the
/// category `item_class`.
#[derive(Clone, PartialEq, Message)]
pub(crate) struct ItemTag {
    #[prost(string, optional, tag = "2")]
    pub(crate) category: Option<String>,
    #[prost(string, optional, tag = "3")]
    pub(crate) internal_name: Option<String>,
}

/// The service methods steamcards calls, as Steam names them.
pub(crate) mod method {
    pub(crate) const BEGIN_QR: &str = "Authentication.BeginAuthSessionViaQR#1";
    pub(crate) const POLL: &str = "Authentication.PollAuthSessionStatus#1";
    pub(crate) const GENERATE_ACCESS_TOKEN: &str = "Authentication.GenerateAccessTokenForApp#1";
    pub(crate) const REVOKE_TOKEN: &str = "Authentication.RevokeToken#1";
    pub(crate) const GET_INVENTORY_ITEMS: &str = "Econ.GetInventoryItemsWithDescriptions#1";
}

/// Decodes a message body, saying which message didn't read.
pub(crate) fn decode<M: Message + Default>(what: &str, body: &[u8]) -> anyhow::Result<M> {
    M::decode(body).map_err(|e| anyhow::anyhow!("Steam sent a {what} that doesn't read: {e}"))
}
