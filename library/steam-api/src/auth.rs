//! Signing in to Steam with a QR code, as the Steam client's own sign-in
//! screen does: Steam's authentication service hands out a code, the Steam
//! app on the user's phone (already signed in) scans and approves it, and
//! Steam hands over tokens. No password is ever typed into steamcards.
//!
//! The same service turns the saved refresh token into a token for
//! steamcommunity.com, and revokes a sign-in when the user signs out.

use std::time::Duration;

use anyhow::{anyhow, bail};
use tokio::time::Instant;

use crate::{
    DEVICE_NAME, EResult,
    cm::{Connection, OS_TYPE, Refused},
    proto::{
        BeginAuthSessionViaQrRequest, BeginAuthSessionViaQrResponse, DeviceDetails,
        GenerateAccessTokenRequest, GenerateAccessTokenResponse, PLATFORM_STEAM_CLIENT,
        PollAuthSessionStatusRequest, PollAuthSessionStatusResponse, RENEWAL_ALLOW, REVOKE_LOGOUT,
        RevokeTokenRequest, RevokeTokenResponse, method,
    },
    token,
};

/// How long to keep showing fresh QR codes before giving up.
const PATIENCE: Duration = Duration::from_secs(30 * 60);
/// Time between polls when Steam doesn't say.
const POLL_EVERY: Duration = Duration::from_secs(5);
/// What the Steam client calls itself when it signs in.
const WEBSITE_ID: &str = "Client";

/// A sign-in Steam approved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approved {
    /// Signs on to Steam for months, and makes the other tokens.
    pub refresh_token: String,
    /// Works on steamcommunity.com for about a day.
    pub access_token: String,
    pub account_name: String,
    pub steam_id: u64,
}

/// A QR code for the Steam app to scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrCode {
    pub url: String,
    /// It's been scanned: the Steam app is asking to approve it.
    pub scanned: bool,
}

/// Signs in with QR codes the Steam app scans. `show` gets each code: a new
/// one replaces the last, and it's shown again once scanned. Codes that
/// expire unscanned are replaced; the sign-in ends when one that was scanned
/// isn't approved, or after half an hour.
pub async fn sign_in_with_qr(
    conn: &Connection,
    mut show: impl FnMut(QrCode),
) -> anyhow::Result<Approved> {
    let give_up = Instant::now() + PATIENCE;
    loop {
        let begun: BeginAuthSessionViaQrResponse = conn
            .call(
                method::BEGIN_QR,
                &BeginAuthSessionViaQrRequest {
                    device_friendly_name: Some(DEVICE_NAME.into()),
                    platform_type: Some(PLATFORM_STEAM_CLIENT),
                    device_details: Some(DeviceDetails {
                        device_friendly_name: Some(DEVICE_NAME.into()),
                        platform_type: Some(PLATFORM_STEAM_CLIENT),
                        os_type: Some(OS_TYPE),
                    }),
                    website_id: Some(WEBSITE_ID.into()),
                },
            )
            .await
            .map_err(|e| anyhow!("Steam wouldn't start a sign-in: {e}"))?;
        let mut url = begun
            .challenge_url
            .filter(|u| !u.is_empty())
            .ok_or_else(|| anyhow!("Steam started a sign-in but sent no QR code"))?;
        let mut pending = Pending::new(begun.client_id, begun.request_id, begun.interval);
        let mut scanned = false;
        show(QrCode {
            url: url.clone(),
            scanned,
        });
        loop {
            if Instant::now() >= give_up {
                bail!("the QR code wasn't scanned in time — try again");
            }
            tokio::time::sleep(pending.every).await;
            match pending.poll(conn).await? {
                Polled::Approved(a) => return Ok(a),
                Polled::Waiting {
                    new_url,
                    interacted,
                } => {
                    let changed = new_url.is_some() || (interacted && !scanned);
                    if let Some(u) = new_url {
                        url = u;
                    }
                    scanned |= interacted;
                    if changed {
                        show(QrCode {
                            url: url.clone(),
                            scanned,
                        });
                    }
                }
                Polled::Ended if scanned => {
                    bail!("the sign-in wasn't approved in the Steam app — try again")
                }
                // Expired before anyone scanned it: a fresh one.
                Polled::Ended => break,
            }
        }
    }
}

/// A token for steamcommunity.com, made from the refresh token. Steam may
/// renew the refresh token as well, and hands the new one back when it does;
/// the old one then stops working, so it has to be saved.
pub async fn web_token(
    conn: &Connection,
    refresh_token: &str,
    steam_id: u64,
) -> anyhow::Result<(String, Option<String>)> {
    let made: GenerateAccessTokenResponse = conn
        .call(
            method::GENERATE_ACCESS_TOKEN,
            &GenerateAccessTokenRequest {
                refresh_token: Some(refresh_token.into()),
                steamid: Some(steam_id),
                renewal_type: Some(RENEWAL_ALLOW),
            },
        )
        .await?;
    let access = made
        .access_token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| anyhow!("Steam sent no token for its community site"))?;
    Ok((access, made.refresh_token.filter(|t| !t.is_empty())))
}

/// Ends a sign-in at Steam's end, as signing out of the Steam client does.
pub async fn revoke(conn: &Connection, refresh_token: &str) -> anyhow::Result<()> {
    conn.call::<_, RevokeTokenResponse>(
        method::REVOKE_TOKEN,
        &RevokeTokenRequest {
            token: Some(refresh_token.into()),
            revoke_action: Some(REVOKE_LOGOUT),
        },
    )
    .await?;
    Ok(())
}

/// A sign-in waiting on the Steam app.
struct Pending {
    client_id: u64,
    request_id: Vec<u8>,
    every: Duration,
}

/// What a poll found.
enum Polled {
    Approved(Approved),
    Waiting {
        /// A new QR code to show in place of the last.
        new_url: Option<String>,
        /// The Steam app has the sign-in open.
        interacted: bool,
    },
    /// Steam ended the sign-in: it expired, or was turned down.
    Ended,
}

impl Pending {
    fn new(client_id: Option<u64>, request_id: Option<Vec<u8>>, interval: Option<f32>) -> Self {
        let every = interval
            .filter(|s| s.is_finite() && *s > 0.0)
            .map_or(POLL_EVERY, Duration::from_secs_f32);
        Self {
            client_id: client_id.unwrap_or_default(),
            request_id: request_id.unwrap_or_default(),
            every,
        }
    }

    async fn poll(&mut self, conn: &Connection) -> anyhow::Result<Polled> {
        let polled = conn
            .call::<_, PollAuthSessionStatusResponse>(
                method::POLL,
                &PollAuthSessionStatusRequest {
                    client_id: Some(self.client_id),
                    request_id: Some(self.request_id.clone()),
                },
            )
            .await;
        let answer = match polled {
            Ok(answer) => answer,
            Err(e) => {
                let ended = e.downcast_ref::<Refused>().is_some_and(|r| {
                    [EResult::FILE_NOT_FOUND, EResult::EXPIRED].contains(&r.eresult)
                });
                return if ended {
                    Ok(Polled::Ended)
                } else {
                    Err(anyhow!("lost touch with Steam while signing in ({e})"))
                };
            }
        };
        if let Some(id) = answer.new_client_id.filter(|&id| id != 0) {
            self.client_id = id;
        }
        let Some(refresh_token) = answer.refresh_token.filter(|t| !t.is_empty()) else {
            return Ok(Polled::Waiting {
                new_url: answer.new_challenge_url.filter(|u| !u.is_empty()),
                interacted: answer.had_remote_interaction.unwrap_or_default(),
            });
        };
        let steam_id = token::read(&refresh_token)
            .map(|t| t.steam_id)
            .ok_or_else(|| anyhow!("Steam approved the sign-in but its token doesn't read"))?;
        Ok(Polled::Approved(Approved {
            refresh_token,
            access_token: answer.access_token.unwrap_or_default(),
            account_name: answer.account_name.unwrap_or_default(),
            steam_id,
        }))
    }
}
