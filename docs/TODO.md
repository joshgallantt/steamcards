# TODO

steamcards farms Steam trading cards from the terminal. It's written from
scratch with [streamdrops](https://github.com/joshgallantt/streamdrops) as
the template: the same language, architecture, file layout and TUI. How Steam
works, and how ASF farms cards, is in
[docs/research/steam-card-farming.md](research/steam-card-farming.md).

`[ ]` to do · `[~]` in progress. What's done comes off the list: git history
keeps it.

---

## Inbox

Things that come up along the way. Add a line here, take it off once it's
done, and move it into a numbered section if it grows.

- [ ] Look at farming faster (2026-09-29, "at some point"). The redesign's
      time-to-finish estimate learns from real drops, so it will show
      whether a change helps.
- [~] Redesign the UI for card farming, not streamdrops' layout: overall
      progress, progress per game, the cards that dropped this session,
      what each card is worth, the session's value, and the time and value
      to finish. See section 2.

---

## 1. Live checks `[~]`

- [ ] Save a real badge page as a test fixture (the current fixtures are
      modelled on ASF's selectors).

## 2. The redesign `[~]`

- [~] Try it on the real account (2026-09-30). The wallet's currency came
      through, prices are looked up at the intended pace, and Steam lists
      unseen card drops with their asset IDs and games (research §6,
      question 1, answered). With nothing unseen it doesn't answer at
      all, which misfiled the first drop; fixed. Still to see: prices in
      another currency (question 2), unlisted cards (question 3), and the
      foil badge page's markup.

## Later

- [ ] Quick-sell: list each card on the market as it drops. The research
      (section 4) covers the request, confirmations, pricing and pacing;
      the redesign leaves room for it.
