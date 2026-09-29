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
- [ ] Stopping and restarting games to shake drops loose (SGI, xPaw,
      steamctl): disputed, ASF calls it a glitch. Maybe later, opt-in, if
      measured.

---

## 1. The workspace `[~]`

- [x] Cargo workspace, lints, clippy.toml, deny.toml, typos.toml, toolchain,
      editorconfig and git hooks, as in streamdrops.
- [x] `xtask`: setup, ci, lint, test, docs, deps, pre-commit, fix, hooks.
- [x] `library/debug-log` and `library/config-file` (the saved sign-in and
      preferences; the file is readable by its owner only).
- [ ] Architecture tests: the dependency rule and the language rules.

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

## 4. The TUI and headless mode `[ ]`

- [ ] Onboarding: welcome, sign in with the Steam app, pick games, start.
- [ ] Dashboard: header, Now panel, farm queue (priority, indifferent,
      skipped, done), details panel, event strip and key hints.
- [ ] Pop-ups: account, sign-in (QR), games, log, details, help and quit.
- [ ] Previews of every screen, as streamdrops' `preview.rs` does.
- [ ] `--headless` prints the same events.

## 5. Live checks `[ ]`

- [x] Connect to a real CM server and start a QR sign-in (this works without
      an account). Done 2026-09-29: Steam's London server answered with a QR
      code, and polling ran cleanly.
- [ ] Sign in with a real account, see its badges, and farm one game until a
      card drops.
- [ ] Save a real badge page as a test fixture (the current fixtures are
      modelled on ASF's selectors).

## 6. Release `[ ]`

- [ ] README, CONTRIBUTING and architecture docs.
- [ ] Install scripts and a Homebrew formula (macOS and Linux), the release
      workflow and CI.

## Later

- [ ] Hours threshold as a setting (ASF's `HoursUntilCardDrops`; 0 farms
      every game one at a time).
- [ ] Refund guard: skip games bought in the last 14 days with under 2 hours
      played. Needs purchase dates from the licence list.
- [ ] Skip private games (ASF asks Steam for the account's private app list).
