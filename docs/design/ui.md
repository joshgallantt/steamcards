# The terminal UI

The dashboard shows how far farming has got, what each game has left, and
the cards that dropped this session, with what they're worth. It replaces
a larger design with many panels and views that proved too much to take in
(git history keeps it, from the commit "Specify the new terminal UI").

The mockups below are the previews' renders
(`cargo test -p terminal-ui previews -- --nocapture`), with their sample
data.

## The dashboard

```
 header       what's happening                     account · appears offline   ? help
 summary      This session  time · cards · value   To go  cards · games · time
              All games     ━━━━━━────  received of all · when done ≈ value
 Games                                       │ This session
   one row per game, in farm order            │   one row per card, newest first
 strip        the latest event, or what a key just did
 footer       keys
```

```mockup 120x30 the dashboard, farming
 steamcards   ● farming Portal 2 · next check in 12m                            cardfarmer · appears offline    ?  help
 This session  1h 35m · 5 cards · £0.84+     To go  15 cards · 5 games · about 8 hours
 All games     ━━━━━━━━━━━━━━━━━━━━━━━━────────────────────────────────────  11 of 28 · when done ≈ £2.03+
╭ Games ────────────────────────────────────────────────────── 5 to go ╮╭ This session ────────────── 5 cards · £0.84+ ╮
│       GAME                                           CARDS   TO COME ││ 14:04 Portal 2  ⠋ which card?              … │
│ ▶ #1  Portal 2                                         1/4     £0.19 ││ 14:01 Portal 2  Atlas, 2nd             £0.06 │
│   #2  Stardew Valley                                   0/4     £0.52 ││ 13:58 Hades     ★ Thanatos             £0.62 │
│       Hades                                            3/4     £0.08 ││ 13:31 Hades     Zagreus, 2nd           £0.08 │
│       Baldur's Gate 3                                  0/3         … ││ 13:02 Hades     Zagreus                £0.08 │
│       The Witcher 3: Wild Hunt                         0/4     £0.40 ││                                              │
│ ✕     Counter-Strike 2                                 0/2           ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
│                                                                      ││                                              │
╰──────────────────────────────────────────────────────────────────────╯╰──────────────────────────────────────────────╯
 14:05  ✓ A card dropped for Portal 2 — 3 to go
  ↑↓  choose   enter  details   1-9  rank   x  skip  │   a  account   g  games   l  log   p  pause   ?  help   q  quit
```

| Part | Shows |
| --- | --- |
| Header | What's happening: farming a game and when its cards are next checked, building hours, waiting while another device plays and when farming carries on after it, paused, reading the badges, nothing to farm, an error and when it's tried again, or the sign-in. Then the account and whether it appears offline. |
| This session | How long this session has played (waiting for another device and pauses left out), then the cards that dropped, each copy counted, and what they're worth. |
| To go | The card drops still to come in the games that will be farmed, how many games, and about how long: learnt from this session's drops, assuming half an hour a drop until two have dropped. |
| All games | Every game with cards: the drops received of all there are, as a gauge. When done: what this session's cards and those still to come will be worth. |
| Games | Each game, in the order it's farmed: ▶ farming, ▷ building hours, #1 its priority, ✕ skipped, ✓ done (shown with c). CARDS is drops received of its total. TO COME is what its drops still to come are worth, at its set's average price. |
| This session | Each card that dropped, newest first: when, the game, the card, what it's worth. A card you already had says so (", 2nd"). ★ is a foil. |
| Strip | The latest event: a drop, a game started, a warning. |

## Values

- Cards are valued at their **market price**: what buyers pay, the lowest
  listing on the Steam market, in the wallet's currency. Prices are looked
  up a game at a time, the game farming first, one request every few
  seconds (see [the research](../research/market-and-session.md)).
- A total with some cards not priced yet ends in `+`: `£0.84+`.
- `…` is a price on its way; `—` is none to be had (nobody's selling it, it
  can't be sold, or the lookup failed).
- A game's cards to come are valued at its set's average price, foils left
  out.

## Duplicates

A game has two measures, and neither stands in for the other: its **drops**
(received of its total: what farming works through) and its **set**
(distinct cards held of the set's size: what a badge needs). The same card
can drop twice. So each card that drops is its own row, a card you already
had says which copy it is, and the set in a game's details shows how many
of each card you have (`×2`), or `—` for none.

## A game's details

`enter` opens the chosen game's details, at any size: what it's doing, its
hours and drops, its set with a count and a price for each card, and its
farm priority. `↑↓` moves to the next game; `PgUp`/`PgDn` scroll when it
doesn't fit, and its bottom edge says how much is below.

```mockup 120x30 a game's details
 steamcards   ● farming Portal 2 · next check in 12m                            cardfarmer · appears offline    ?  help

                     ╭ Portal 2 ──────────────────────────────────────────────────────────────────╮
                     │                                                                            │
                     │  ▶ Farming now                                                             │
                     │  Played on its own, so its cards can drop.                                 │
                     │                                                                            │
                     │  Hours   5.2h on record                                                    │
                     │  Drops   1 of 4 · 3 to come · ≈ £0.19                                      │
                     │  Set     3 of 8 cards · 1 spare                                            │
                     │                                                                            │
                     │    Atlas          ×2    £0.06                                              │
                     │    P-Body         ×1    £0.05                                              │
                     │    Wheatley        —    £0.07                                              │
                     │    GLaDOS          —    £0.09                                              │
                     │    Chell          ×1    £0.06                                              │
                     │    Space Core      —    £0.05                                              │
                     │    Cave Johnson    —    £0.08                                              │
                     │    Turret          —    £0.04                                              │
                     │                                                                            │
                     │  ── Farm priority ───────────────────────────────────────────────────────  │
                     │   ◉ Priority #1    1-9   farmed first, in rank order                       │
                     │   ○ Indifferent     0    after your priorities                             │
                     │   ○ Skip            x    never farmed                                      │
                     │                                                                            │
                     ╰───────────────────────────────────  ↑↓  previous / next game    esc  close ╯


 14:05  ✓ A card dropped for Portal 2 — 3 to go
 ▸ Portal 2 is now priority #1.
```

## Small windows

From 90 columns, the games and this session's cards sit side by side.
Narrower, this session's newest cards sit under the games, when there are
14 rows for them; shorter still, only the games show, and the summary
still has this session's count and value.

```mockup 80x24 80 columns
 steamcards   ● farming Portal 2                    cardfarmer · appears offline
 This session  1h 35m · 5 cards · £0.84+     To go  15 cards · about 8 hours
 All games     ━━━━━━━━━━━━━────────────────────  11 of 28 · when done ≈ £2.03+
╭ Games ────────────────────────────────────────────────────────────── 5 to go ╮
│       GAME                                                   CARDS   TO COME │
│ ▶ #1  Portal 2                                                 1/4     £0.19 │
│   #2  Stardew Valley                                           0/4     £0.52 │
│       Hades                                                    3/4     £0.08 │
│       Baldur's Gate 3                                          0/3         … │
│       The Witcher 3: Wild Hunt                                 0/4     £0.40 │
│ ✕     Counter-Strike 2                                         0/2           │
│                                                                              │
│                                                                              │
│                                                                              │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯
╭ This session ────────────────────────────────────────────── 5 cards · £0.84+ ╮
│ 14:04 Portal 2  ⠋ which card?                                              … │
│ 14:01 Portal 2  Atlas, 2nd                                             £0.06 │
│ 13:58 Hades     ★ Thanatos                                             £0.62 │
│ 13:31 Hades     Zagreus, 2nd                                           £0.08 │
╰─────────────────────────────────────────────────────────────────── 1 earlier ╯
 14:05  ✓ A card dropped for Portal 2 — 3 to go
  ↑↓  choose   enter  details  │   a  account   p  pause   ?  help   q  quit
```

```mockup 60x16 the smallest
 steamcards   ● farming Portal 2             appears offline
 This session  1h 35m · 5 cards · £0.84+     To go  15 cards
 All games     ━━━━━────────  11 of 28 · when done ≈ £2.03+
╭ Games ────────────────────────────────────────── 5 to go ╮
│       GAME                               CARDS   TO COME │
│ ▶ #1  Portal 2                             1/4     £0.19 │
│   #2  Stardew Valley                       0/4     £0.52 │
│       Hades                                3/4     £0.08 │
│       Baldur's Gate 3                      0/3         … │
│       The Witcher 3: Wild Hunt             0/4     £0.40 │
│ ✕     Counter-Strike 2                     0/2           │
│                                                          │
│                                                          │
╰──────────────────────────────────────────────────────────╯
 14:05  ✓ A card dropped for Portal 2 — 3 to go
  ↑↓  choose   enter  details  │   ?  help   q  quit
```

## Nothing is cut off

- Each piece of text comes in a few lengths, and the longest that fits is
  drawn. What gives way first: in the header, help, then the next check,
  then the account's name, then the game's name (shortened, then gone);
  "appears offline" goes last.
- Only a name is shortened, at the end of a word, with "…": a game's in its
  row, a card's in the list, a log line's end. The details show a game's
  name whole.
- A list that doesn't fit says how much more there is.
- Pop-ups clear what's behind them, bar the top line and the bottom two, so
  no half-hidden words show at their edges.
- A test renders every state at sizes from 60×16 to 240×70 and fails on any
  line wider than its area, a name cut mid-word, or a header that has lost
  "appears offline".

## Keys

| Key | Does |
| --- | --- |
| ↑ ↓, PgUp PgDn, Home End | choose a game |
| enter | the chosen game's details |
| 1–9 | rank it priority #1–#9 |
| 0, Backspace, Delete | back to indifferent |
| x | skip it: never farmed |
| o | open its card page |
| c | show or hide finished games |
| g | games: pick priority games, only priority, shake drops loose |
| a | account: sign in or out, appear offline |
| l | the log |
| p | pause, or carry on |
| ? | help |
| q | quit (asks while farming); ctrl-c quits at once |

## Later

Quick-sell (listing each card on the market as it drops) is researched in
[the research](../research/market-and-session.md), section 4, and the
market component models what it will need. It would add a sale column to
this session's cards and its own settings.
