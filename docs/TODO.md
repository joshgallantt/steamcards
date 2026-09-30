# TODO

steamcards farms Steam trading cards from the terminal. It's written from
scratch with [streamdrops](https://github.com/joshgallantt/streamdrops) as
the template: the same language, architecture, file layout and TUI. How Steam
works, and how ASF farms cards, is in
[docs/research/steam-card-farming.md](research/steam-card-farming.md).

`[ ]` to do · `[~]` in progress · `[x]` done

---

## Inbox

Things that come up along the way. Add a line here, check it off when it's
done, and move it into a numbered section if it grows.

- [x] One Steam account only, by choice: no multi-account farming, now or
      later.
- [x] macOS and Linux first. Windows can come later: its installer, its CI
      job, and a check that everything still builds there.
- [x] Sign in with a QR code only: the Steam app, already signed in on your
      phone, approves steamcards. No password is ever typed in. "Sign in
      with Steam" in a browser only tells a site who you are, so it can't
      sign steamcards on to Steam.
- [x] Appear offline while farming, as an option, on by default. Steam
      counts the games either way; offline, friends don't see them.
- [x] Cross-check ASF's farming against the other farmers and Steam's own
      pages (2026-09-29): see the research doc. Agreed on one game at a time
      for drops, 3 hours, 32 together only to build hours.
- [x] Build the domain from its entities: `SteamLibrary`, `Game`, `Card`, in a
      `library` component that farming works through.
- [x] Keep the computer awake while games play: `caffeinate` on macOS,
      `systemd-inhibit` on Linux, let go when there's nothing to farm
      (2026-09-29).
- [x] First real run (2026-09-29): signed in with the Steam app, signed on
      with the saved sign-in, read the badges and started farming. A first run's debug log couldn't open before the config's
      folder existed; fixed.
- [ ] Keeping awake as a setting, for anyone who'd rather it didn't.
- [ ] Stopping and restarting games to shake drops loose (SGI, xPaw,
      steamctl): disputed, ASF calls it a glitch. Maybe later, opt-in, if
      measured.
- [ ] Look at farming faster (2026-09-29, "at some point"). The redesign's
      time-to-finish estimate learns from real drops, so it will show
      whether a change helps.
- [ ] Some games read 0.0h on the badge page but have drops received
      (one had 6 of 12). Check what their badge rows say. If Steam
      leaves their hours out, farming builds 3 hours for games that may not
      need it.
- [~] Redesign the UI for card farming, not streamdrops' layout: overall
      progress, progress per game, the cards that dropped this session,
      what each card is worth, the session's value, and the time and value
      to finish. See section 7.

---

## 1. The workspace `[x]`

- [x] Cargo workspace, lints, clippy.toml, deny.toml, typos.toml, toolchain,
      editorconfig and git hooks, as in streamdrops.
- [x] `xtask`: setup, ci, lint, test, docs, deps, pre-commit, fix, hooks.
- [x] `library/debug-log` and `library/config-file` (the saved sign-in and
      preferences; the file is readable by its owner only).
- [x] Architecture tests: the dependency rule and the language rules.

## 2. `library/steam-api` `[x]`

- [x] A connection to a Steam CM server over WebSocket: framing, `Multi`
      unpacking, jobs, heartbeat, logon with a refresh token, games played,
      and the notifications that matter (playing blocked, new items, logged
      off).
- [x] QR sign-in: `BeginAuthSessionViaQR`, then poll until approved. Handles
      a rotated QR code, an expired one and a declined one.
- [x] `Session`: the saved sign-in, the "Steam rejected it" flag, the live
      connection, and the web token for steamcommunity.com.
- [x] Badge pages and game card pages, parsed with ASF's selectors.
- [x] Tests against a fake CM server and a local stand-in for the community
      site. No test talks to Steam.

## 3. Components `[x]`

- [x] `account`: get, refresh, link (QR) and unlink the one Steam account.
- [x] `preferences`: priority games (ranked), skipped games, "only priority",
      "appear online".
- [x] `library`: the Steam library's games, their card drops and card sets,
      read from the badge and card pages.
- [x] `farming`: one game at a time for its cards once it has 3 hours;
      below that, up to 32 together to build hours (counted locally, as the
      badge pages lag). Look every 15 minutes, 5 with one drop left, and at
      once when Steam says new items arrived. Wait while another device
      plays, then a minute more. A game that drops nothing for 10 hours goes
      behind the others. Sale-event badges are never played. Another session
      taking over stops farming rather than fighting it.
- [x] Acceptance tests on paused time, as streamdrops' mining tests do.

## 4. The TUI and headless mode `[x]`

- [x] Onboarding: welcome, sign in with the Steam app, pick games, start.
- [x] Dashboard: header, Now panel, farm queue (priority, indifferent,
      skipped, done), details panel with the card set, event strip and key
      hints.
- [x] Pop-ups: account (sign out, appear offline), sign-in (QR), games, log,
      details, help and quit.
- [x] Previews of every screen, as streamdrops' `preview.rs` does.
- [x] `--headless` prints the same events.
- [ ] Screenshots for the README, as streamdrops' `xtask screenshots` makes
      them.

## 5. Live checks `[~]`

- [x] Connect to a real CM server and start a QR sign-in (this works without
      an account). Done 2026-09-29: Steam's London server answered with a QR
      code, and polling ran cleanly.
- [x] Drive the real TUI to a live QR code in a terminal (2026-09-29).
- [x] Sign in with a real account, see its badges, and farm one game until a
      card drops. Signed in, read the badges and farmed on 2026-09-29: games
      took turns alone and cards dropped about every 30 minutes.
- [ ] Save a real badge page as a test fixture (the current fixtures are
      modelled on ASF's selectors).

## 6. Release `[ ]`

- [x] README, CONTRIBUTING and architecture docs.
- [ ] Install scripts and a Homebrew formula (macOS and Linux), the release
      workflow and CI.

## 7. The redesign `[~]`

- [x] Research where prices, the wallet's currency, drop asset IDs and
      completion estimates come from, and what quick-sell will need:
      [docs/research/market-and-session.md](research/market-and-session.md).
- [x] Design: five designs from different angles, judged, and one spec
      made from them (`docs/design/ui.md`). Its questions for you (§9)
      are still open; the screens can start from its defaults.
- [x] Domain first (2026-09-30): the farming session and its card drops,
      one drop per copy, each named where Steam or its card page can tell
      and numbered from the account's counts, and the time to finish
      learnt from real drops; a `market` component with money, prices,
      the wallet and the value basis, and what the cards held and still
      to drop are worth.
- [x] Data (2026-09-30): the wallet's currency and the drops' asset IDs
      over the CM connection, set prices from the market at a polite pace
      (one queue, 5 s apart; Steam's pause and the prices outlast a
      restart), fee maths.
- [x] The screens' foundations (2026-09-30): text that's never cut, the
      spec's formats, the size classes and every region's ladder, the view
      models, the spec's data set as fixtures, and golden tests that hold a
      screen to its mockup. The market is in the app: the composition root
      builds `MarketComponent`, with `prices.json` beside the config, and
      the keys b, t, h and m.
- [ ] The new screens, drawn on the foundations, each held to its mockup,
      and a check that nothing is cut off at any size (`docs/design/ui.md`,
      §8).
- [x] The pop-ups and onboarding (2026-09-30): the game's details, this
      session's cards, the market, games & settings, the account, signing
      in, the log, help and quitting, each over whole panels with what's
      behind faded; the Start step sizing up the job; the Welcome whole at
      60 × 16. Each mocked one held to its mockup.
- [x] Review the domain and data adversarially (2026-09-30): three
      reviewers, on fidelity to the spec, Steam's own behaviour and the
      engineering. Fixed: each drop is checked against its game's counts,
      a read's drops have their card page read, copies (foils too) are
      numbered from the account's counts, the market's failures are told
      from a market it couldn't ask, and another account's sign-in ends
      the session.
- [ ] Try it on the real account. The first real drops and prices answer
      the research's open questions 1 to 3 (§6); `STEAMCARDS_DEBUG` keeps
      what Steam announces of each drop, and the foil badge page's markup
      is still modelled, not seen.
- [ ] If the first drops show Steam's announcements leave asset IDs out
      (research §6, question 1), build the inventory fallback (§2.3 A):
      the community inventory read over CM at sign-on, then diffed as
      drops come. Until then a drop Steam doesn't name goes by its card
      page, or reads "couldn't tell which card".

## Later

- [ ] Quick-sell: list each card on the market as it drops. The research
      (section 4) covers the request, confirmations, pricing and pacing;
      the redesign leaves room for it.

- [ ] Hours threshold as a setting (ASF's `HoursUntilCardDrops`; 0 farms
      every game one at a time).
- [ ] Refund guard: skip games bought in the last 14 days with under 2 hours
      played. Needs purchase dates from the licence list.
- [ ] Skip private games (ASF asks Steam for the account's private app list).
