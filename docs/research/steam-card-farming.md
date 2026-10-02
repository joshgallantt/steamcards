# Research: farming Steam trading cards

What the existing farmers do, what Steam itself says, and what steamcards
does as a result. First checked 2026-09-28; the farming rules were
cross-checked against five other farmers and Valve's pages on 2026-09-29.

---

## The field

| Project | Last active | Licence | How it plays | What we took |
| --- | --- | --- | --- | --- |
| [ArchiSteamFarm](https://github.com/JustArchiNET/ArchiSteamFarm) (ASF) | 2026-09-28 | Apache-2.0 | Steam's CM servers, via SteamKit | The badge selectors, the two-phase algorithm, most timings. Credited in NOTICE. |
| [Steam Game Idler](https://github.com/zevnda/steam-game-idler) (SGI) | 2026-09-25 | Elastic 2.0 (source-available) | CM servers via SteamKit, or Steamworks processes | Behaviour only, no code: the 3-hour default and one game at a time since 6.2.0, and not reconnecting after `LogonSessionReplaced`. |
| [xPaw/Steam-Card-Farmer](https://github.com/xPaw/Steam-Card-Farmer) | 2026-09-19 | MIT | CM via node-steam-user | Staying offline by never setting a persona state; a distinct login ID. |
| [steamctl](https://github.com/ValvePython/steamctl) `idle-cards` | 2020-12 | MIT | CM via the Python `steam` lib | Waiting while the account plays elsewhere. |
| [Steam Tools NG](https://github.com/calendulish/steam-tools-ng) | 2024-11 | GPL-3.0 | Steamworks processes | Nothing new. |
| [Idle Master](https://github.com/jshackles/idle_master) / [Extended](https://github.com/JonasNilson/idle_master_extended) | archived | GPL-2.0 | Steamworks processes (`steam-idle.exe`) | The 5-minute look when one drop is left. |
| [SteamKit](https://github.com/SteamRE/SteamKit) | 2026-09-16 | LGPL-2.1 | — | Protocol knowledge: message numbers, `EResult`s, `EOSType`s. |
| [steam-vent](https://codeberg.org/steam-vent/steam-vent) | 2026-06-28 | MIT | — | A reference for the CM handshake. steamcards has its own client instead of depending on it. |
| [SteamDatabase/Protobufs](https://github.com/SteamDatabase/Protobufs) | — | — | — | Valve's own `.proto` files: every message's fields and numbers. |

Farmers that run Steamworks processes need the Steam client running and
signed in. The CM ones, ASF's approach and steamcards', need nothing but a
sign-in: Steam is simply told which games are being played.

---

## Signing in: a QR code, and nothing else

The Steam app on the user's phone, already signed in, scans a QR code and
approves steamcards. No password is ever typed in. This is how the Steam
client's own sign-in screen works (and ASF's `LoginWithQrCode`):

1. On a CM connection that isn't signed on, call
   `Authentication.BeginAuthSessionViaQR#1` as a Steam client
   (`platform_type` 1, `website_id` "Client", device name "steamcards").
2. Show `challenge_url` as a QR code.
3. Poll `Authentication.PollAuthSessionStatus#1` every `interval` seconds. It
   may send a `new_challenge_url` (show it instead) and
   `had_remote_interaction` (it's been scanned). It ends with a refresh
   token, an access token and the account name.
4. A code that expires unscanned is replaced; one that was scanned and then
   ends wasn't approved.

"Sign in with Steam" in a browser (OpenID) was considered and ruled out: it
only tells a website which account someone has, and hands over nothing that
can sign on to Steam or read the badges.

**Signing on.** `ClientLogon` takes the refresh token in its `access_token`
field, with a login ID kept across sign-ins (so Steam sees one computer) and
the OS as SteamKit's `EOSType` (macOS `-102`, Linux `-203`). Refresh tokens
last about 200 days.

**steamcommunity.com.** `Authentication.GenerateAccessTokenForApp#1` turns
the refresh token into a token for the site; Steam may renew the refresh
token at the same time, and the old one then stops working, so it's saved.
The site takes it as a cookie, `steamLoginSecure = "{steamid64}||{token}"`,
with a random `sessionid`. A page shown with `g_steamID = false` means the
token wasn't taken: make a new one, once.

**Signing out** revokes the refresh token (`Authentication.RevokeToken#1`),
as signing out of the Steam client does.

---

## The library: games, card drops, cards

**Badge pages** (`/profiles/{id}/badges?l=english&p=N`, ASF's selectors, which
ASF keeps in step with Steam's site):

- a row is `div.badge_row_inner`; one without `div.card_drop_info_dialog`
  is a badge that isn't a game's, and is skipped;
- the app ID is the dialog's id, split on `_`, fifth part;
- drops left: the number in `span.progress_info_bold` ("No card drops
  remaining" has none);
- drops so far: `div.card_drop_info_header` ("Card drops received: 2");
- hours: `div.badge_title_stats_playtime`;
- name: the last `div.card_drop_info_body` ("…by playing Portal 2.");
- the page count: the last `a.pagelink`.

Idle Master reads the app ID from the row's `a.badge_row_overlay` link
instead, and SGI from its `steam://run/` link: fallbacks if the dialog ever
goes.

**A game's own card page** (`/gamecards/{appid}?l=english`) has the same
drop count, and the card set: each child of `div.badge_card_set_cards` whose
class starts `badge_card_set_card`, marked `unowned` when the account has
none, with its quantity in `div.badge_card_set_text_qty` (ASF's
`GetCardCountForGame` uses the same selector). Team Fortress 2, Dota 2 and
Counter-Strike 2 (440, 570, 730) can show "no drops" on the badge page when
they have some: look at their own page (ASF's `UntrustedAppIDs`).

**Being polite:** 300 ms between requests (ASF's `WebLimiterDelay`), a 429
or 5xx asked again with a back-off, and a page that didn't load is never
taken to mean "no drops left".

---

## What Steam itself says

- **Valve's Trading Cards FAQ:** cards come from playing; about half a set
  drops; free-to-play games give a drop per roughly $9 spent. Nothing about
  hours, playing several games at once, or appearing offline.
- **Steamworks:** each game's developer sets the playtime a card takes. This
  is the only official timing rule found.
- **No drops, ever:** limited accounts; games marked private; free
  promotional copies; free-to-play games without purchases; family-shared
  games (only the owner gets drops); games Valve hasn't yet enabled cards
  for.
- **The "2 hours before drops" rule** was never documented by Valve. It
  appeared in 2015 around refunds; ASF made it 3 hours in 2017, and every
  farmer since treats it as an observation.

---

## Farming: where the farmers agree, and where they don't

| | ASF | SGI | xPaw | steamctl | STNG | Idle Master |
| --- | --- | --- | --- | --- | --- | --- |
| Hours before a game is farmed alone | 3 | 3 | 3 (180 min) | none | 2 | 2 |
| Games at once, for drops | 1 | 1 (since 6.2.0) | up to 32 | up to 32 | up to 50 | 1 (default) |
| Games at once, to build hours | up to 32 | up to 32 | up to 32 | — | — | up to 30 |
| Stops and restarts games | no ("a glitch") | yes, every 5 min | yes | yes | yes | fast mode |
| Looks for drops | every 15 min + on new items | every 5–8 min, every page | on new items, every 3 h | on new items | per game | every 15 min, 5 min with one left |
| Playing elsewhere | stop; 60 s after | stop; 60 s after | stop; at once | wait | — | — |

**Agreed, and what steamcards does:**

- Cards drop for one game at a time. ASF measures several at once as "close
  to zero", and SGI switched to one at a time on 2026-08-06 after three weeks
  of 32 at once.
- Games short of the hours threshold are played together, up to 32 (Steam's
  limit), until the one with the most hours gets there. The hours are
  counted locally: the badge pages lag about half an hour.
- The threshold is 3 hours (ASF, SGI, xPaw).
- Look at the game being farmed every 15 minutes, as soon as Steam says new
  items arrived, and every 5 minutes when one drop is left.
- Stop while the account plays on another device, and carry on 60 seconds
  after it stops. Announce nothing while blocked.
- A session replaced by another with the same login (`LogonSessionReplaced`)
  doesn't reconnect: SGI found its own clients knocking each other off
  every couple of seconds.
- Appear offline by default. ASF recommends it for main accounts, xPaw and
  steamctl never go online, and all say playtime still counts.

**Disputed, so off unless asked for:** stopping and restarting games to
shake drops loose. SGI, xPaw, steamctl and Idle Master's fast mode do it;
ASF calls it exploiting a Steam glitch that may break Steam's online conduct
rules, and refuses. Nobody publishes measurements. steamcards does it as
SGI does, every 5 minutes, for the game farmed alone, once the user turns it
on (2026-10-02).

**steamcards' own choices:**

- **Order:** the user's priority games first, then games whose cards can drop
  now, fewest drops left first (SGI's order: games finish sooner), then games
  building hours, most hours first. A priority game still short of the
  threshold leads the group building hours.
- **A game that drops nothing** for 10 hours (ASF's `MaxFarmingTime`) goes
  behind the others; the second time, it's left alone for the rest of the
  session, with a hint at why (family-shared, free-to-play, private).
- **Sale-event badges** (ASF's `SalesBlacklist`) are never played: those
  cards come from taking part in a sale.
- **With nothing to farm,** sign off, and look again every 8 hours (ASF's
  `IdleFarmingPeriod`) or as soon as the user's choices change.

All of these are named, with their sources, in
`component/farming/domain/src/rules.rs`.

**Playing on another device.** Only one of an account's sessions plays at a
time. What Steam does, as SteamKit and ASF say, and as a live run in
September 2026 showed:

- As a session signs on, Steam says whether another device is playing, and
  what (`ClientPlayingSessionState`). It did at each of 43 sign-ons, within
  a tenth of a second. It says so again whenever another session starts or
  stops playing.
- While another device plays, a session that says it's playing is signed
  off at once. SteamKit: "While blocked, sending ClientGamesPlayed message
  will log you off with LoggedInElsewhere result."
- Starting a game on one device while a session elsewhere plays, the Steam
  client offers to close the other game. That signs the playing session off
  with `LoggedInElsewhere` too. ASF: "This result directly indicates that
  playing was blocked when we got (forcefully) disconnected."

So steamcards waits for Steam's word at sign-on before it plays, and plays
nothing while another device does. Signed off by `LoggedInElsewhere`, it
signs on again and stays signed on, playing nothing, until Steam says the
other device stopped, then carries on a minute later. If Steam doesn't say
the other device is playing, its game gets 5 minutes to start (an update,
or shaders, can hold it up) before steamcards plays again.

**Later, maybe:** the threshold as a setting (0 for accounts that aren't held
back); telling such an account apart on its own (a card that drops before 3
hours); a refund guard (skip games bought in the last 14 days with under 2
hours played, which needs purchase dates).

---

## Licensing

- ASF is Apache-2.0: its selectors, rules and timings are ported with
  credit, in NOTICE.
- SGI is Elastic 2.0, Idle Master and Steam Tools NG are GPL: read for
  behaviour and facts, nothing copied.
- SteamKit is LGPL: message numbers and enum values are facts about Steam's
  protocol, not its code.
- steam-vent and xPaw's farmer are MIT, like steamcards.
