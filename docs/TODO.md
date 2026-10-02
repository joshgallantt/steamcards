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
- [x] Windows, as streamdrops has it (2026-10-01): `install.ps1` and
      `uninstall.ps1` with their tests, CI's tests on Windows and
      PSScriptAnalyzer on the scripts, the Windows release zip, and the README
      and CONTRIBUTING.md's Windows instructions. CI passed on Windows,
      under Windows PowerShell 5.1 and PowerShell 7 (pull request #1).
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
      (2026-09-29). Taken out again (2026-10-01): the computer's own sleep
      settings decide.
- [x] First real run (2026-09-29): signed in with the Steam app, signed on
      with the saved sign-in, read the badges and started farming. A first
      run's debug log couldn't open before the config's
      folder existed; fixed.
- [ ] Stopping and restarting games to shake drops loose (SGI, xPaw,
      steamctl): disputed, ASF calls it a glitch. Maybe later, opt-in, if
      measured.
- [ ] Look at farming faster (2026-09-29, "at some point"). The redesign's
      time-to-finish estimate learns from real drops, so it will show
      whether a change helps.
- [x] Some games read 0.0h on the badge page but had drops received
      (one had 6 of 12). The hours were right: those games had never been
      played. The drops were wrong: a game never played has no "received"
      line in its card drop details, and the parser read the first number
      there, which is what's left. Fixed (2026-09-30): it reads 0 of 6.
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
- [x] Screenshots for the README, drawn from the previews by `cargo xtask
      screenshots`, and checked by CI against what the UI draws
      (2026-09-30).

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

## 6. Release `[~]`

- [x] README, CONTRIBUTING and architecture docs, set out as streamdrops'
      are, with a code of conduct, a security policy, issue and pull request
      templates (2026-09-30).
- [x] Install and uninstall scripts (macOS and Linux) with their own tests,
      a Homebrew formula in the repository as its own tap, the release
      workflow and `cargo xtask release`, CI on every push and pull request,
      Dependabot, and rulesets for `main` and tags (2026-09-30).
- [x] Nothing personal in the history (2026-09-30): no account names,
      local paths or details of a real account. The history was rewritten
      and the GitHub repository recreated from it, so no old copy is left.
      Keep it that way: made-up names in tests, fixtures, docs and
      screenshots, and check a debug log before quoting it.
- [ ] CI on GitHub: Actions won't start jobs until the account's billing
      is sorted (Settings → Billing and plans). Once public, they run free.

### Going public

In this order, on the day:

- [ ] Check the history once more: `git log --all -p` for anything
      personal. Rewriting it is only simple before anyone has cloned it.
- [ ] Make the repository public (Settings → General → Danger zone).
- [ ] Turn on private vulnerability reporting, which the security policy's
      "Report a vulnerability" link needs:
      `gh api -X PUT repos/joshgallantt/steamcards/private-vulnerability-reporting`.
- [ ] `cargo xtask protect`: the rulesets for `main` and tags. GitHub applies
      them to a private repository only with GitHub Pro.
- [x] The first release, v0.1.0 (2026-10-02): `cargo xtask release 0.1.0`.
      It filled in the Homebrew formula's checksums.
- [ ] Check the one-liners install and uninstall a release: the release
      workflow does it once the repository is public, and skipped it while
      it's private.
- [x] Take the "hasn't had its first release yet" paragraph out of the
      README's Install section.
- [ ] Check the README's screenshots show on GitHub.

## 7. The redesign `[~]`

- [x] Research where prices, the wallet's currency, drop asset IDs and
      completion estimates come from, and what quick-sell will need:
      [docs/research/market-and-session.md](research/market-and-session.md).
- [x] Design: five designs from different angles, judged, and one spec
      made from them. Built, then replaced by a simpler one (below).
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
- [x] Screens, first go (2026-09-30): a large design, with a progress
      panel, a track of the day, value bases, a market view and golden
      tests. Stopped: "way too complex and confusing". It's in git history.
- [x] Screens, simplified (2026-09-30): one dashboard, as picked: a
      summary (this session, what's to go, every game, the value when
      done), the games beside this session's cards, and a game's details
      on enter, with a count and a price for each card. Market prices
      only. Nothing cut off: every state tested at sizes from 60×16 to
      240×70 ([docs/design/ui.md](design/ui.md)).
- [x] Review the domain and data adversarially (2026-09-30): three
      reviewers, on fidelity to the spec, Steam's own behaviour and the
      engineering. Fixed: each drop is checked against its game's counts,
      a read's drops have their card page read, copies (foils too) are
      numbered from the account's counts, the market's failures are told
      from a market it couldn't ask, and another account's sign-in ends
      the session.
- [~] Try it on the real account (2026-09-30). The wallet's currency came
      through, prices are looked up at the intended pace, and Steam lists
      unseen card drops with their asset IDs and games (research §6,
      question 1, answered). With nothing unseen it doesn't answer at
      all, which misfiled the first drop; fixed. Still to see: prices in
      another currency (question 2), unlisted cards (question 3), and the
      foil badge page's markup.
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
