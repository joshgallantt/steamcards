# Research: farming Steam trading cards

Checked 2026-09-28. What the existing farmers do, which one to learn from,
and what we build on in Rust.

---

## The field

| Project | Stars | Last commit | Last release | Lang | Licence | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| [JustArchiNET/ArchiSteamFarm](https://github.com/JustArchiNET/ArchiSteamFarm) | 13.7k | 2026-09-28 | 6.3.10.3 (2026-09-26) | C# | Apache-2.0 | **The reference.** Actively maintained. Logic may be ported with attribution. |
| [zevnda/steam-game-idler](https://github.com/zevnda/steam-game-idler) | 741 | 2026-09-25 | 6.2.7 | TS (Tauri) | Elastic 2.0 | Source-available, not open source. Don't copy from it. |
| [xPaw/Steam-Card-Farmer](https://github.com/xPaw/Steam-Card-Farmer) | 61 | 2026-09-19 (dependabot) | v3.0.0 (2020) | TS/Node | MIT | Effectively dormant. |
| [SteamRE/SteamKit](https://github.com/SteamRE/SteamKit) | 3.2k | 2026-09-16 | — | C# | LGPL-2.1 | ASF's protocol layer. Port ideas, not code. |
| [DoctorMcKay/node-steam-session](https://github.com/DoctorMcKay/node-steam-session) | 198 | 2025-12 | — | JS | MIT | A clean reference for the QR flow. |
| [steam-vent](https://codeberg.org/steam-vent/steam-vent) | — | 2026-06-28 | 0.5.0 (2026-04) | Rust | MIT | **The only serious Rust CM client.** The GitHub mirror is archived, so use Codeberg. |

The `steam-farming` and `card-farming` GitHub topics hold only 0–9-star
repos, some of which look like malware bait. Ignore them.

---

## Signing in with a QR code

steam-vent can log in with credentials or a refresh token, but it has no QR
helper. We add one, as ASF does (`Steam/Bot.cs`, `LoginWithQrCode`):

1. On the unauthenticated CM connection, call
   `Authentication.BeginAuthSessionViaQR#1` with a device friendly name and
   `platform_type = SteamClient`.
2. Draw `challenge_url` as a QR code, the same way streamdrops draws Twitch's.
   The user scans it in the Steam mobile app.
3. Poll `Authentication.PollAuthSessionStatus#1` every `interval` seconds.
   It may return a `new_challenge_url`: redraw the QR. It finishes with
   `refresh_token`, `access_token` and `account_name`.
4. Save the refresh token. It lasts about 200 days.
5. Log on to the CM with `login_with_refresh_token`. ClientLogon takes the
   refresh token in its `access_token` field.

**Web session, for scraping badges.** Call
`Authentication.GenerateAccessTokenForApp#1` with the refresh token. Steam
may rotate the refresh token here, and a rotated one must be saved. Then set
cookies on `steamcommunity.com`:

- `steamLoginSecure = "{steamid64}||{access_token}"`
- a random `sessionid`

There is no separate web login. Refresh the access token about 5 minutes
before the JWT's `exp`.

---

## Finding games with drops left (`Steam/Cards/CardsFarmer.cs`)

- Fetch `https://steamcommunity.com/profiles/{id}/badges?l=english&p=N`. The
  page count comes from the last `a.pagelink`.
- For each `div.badge_row_inner`:
  - **appID:** the id of `div.card_drop_info_dialog`, split on `_`, index 4
  - **drops remaining:** the digits in `span.progress_info_bold`; missing means 0
  - **hours:** `div.badge_title_stats_playtime`
  - **badge level:** `div.badge_info_description > div`
- Re-check a single game at `/gamecards/{appid}?l=english` after each farming
  period. AppIDs 440, 570 and 730 are unreliable on the badge page, so always
  re-check those individually.
- Rescan every badge page every 8 hours, after each full loop, and when a new
  item notification arrives.

---

## Farming algorithm

Send `CMsgClientGamesPlayed` (ASF uses `EMsg.ClientGamesPlayedWithDataBlob`)
with up to **32** app IDs. That is Steam's limit.

- **Simple** (no hours threshold): play one game until it has no drops left.
- **Complex** (ASF's default, with a threshold of 3 hours):
  1. Play each game already past the threshold, one at a time.
  2. Play up to 32 below the threshold together, highest hours first, until
     they cross it.
  3. Go back to step 1.
- Re-check every 15 minutes, or sooner when Steam's items notification
  arrives (`UserNotifications`).
- Give up on a game after 10 hours with no progress.
- **Order:** the user's priority list first, then a sort such as most drops
  left, fewest hours or name. This maps directly onto streamdrops' priority
  games.
- **Refund guard:** skip games owned for under 14 days with under 2 hours
  played, so farming doesn't cost the user their refund.
- **Playing elsewhere:** `PlayingSessionState { playing_blocked }` means the
  user is playing on another device. Stop farming. When they finish, wait 60
  seconds before resuming so we don't kick them off.

## Being polite to Steam

- Leave 300 ms between web requests to a domain.
- Leave 10 s between logins.
- Retry with backoff. Treat a 429 as "slow down", not as failure.
- Stop after repeated `AccessDenied` or an invalid token, and ask the user to
  sign in again.
- Use a stable machine name and login ID for the persistent session. Scrape
  with `l=english`.

---

## Licensing

- ASF is Apache-2.0. Porting its logic, such as the selectors and the
  algorithm, is fine with a NOTICE and credit.
- SteamKit is LGPL: learn from it, don't copy it.
- steam-game-idler is Elastic 2.0: don't copy it.
- steam-vent is MIT, the same as this project.
