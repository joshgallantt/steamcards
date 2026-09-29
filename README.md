# steamcards

[![macOS](https://img.shields.io/badge/macOS-supported-2ea44f)](#install)
[![Linux](https://img.shields.io/badge/Linux-supported-2ea44f)](#install)
[![Licence: MIT](https://img.shields.io/badge/licence-MIT-blue)](LICENSE)
[![unsafe: forbidden](https://img.shields.io/badge/unsafe-forbidden-2ea44f)](CONTRIBUTING.md)
![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange?logo=rust)

**Get your Steam trading cards without playing for them.** steamcards plays
your games in the background, so their cards drop while you get on with your
day. Nothing is installed or launched: Steam is simply told what's being
played, the way the Steam client tells it.

## Features

- **Sign in with your phone:** scan a QR code with the Steam app. No
  password is typed in.
- **Your games, in your order.** Rank the games you want cards from first;
  the rest follow, the ones closest to dropping first.
- **Farms the way that works.** Cards drop for one game at a time once it has
  3 hours on record, so games short of that are played together, up to 32,
  to build hours. The rules are cross-checked against ArchiSteamFarm, Steam
  Game Idler and the other farmers: see
  [the research](docs/research/steam-card-farming.md).
- **Appears offline** while farming, if you like (it does by default): your
  friends don't see a pile of games, and the cards drop just the same.
- **Steps aside** while you play on another device, and carries on a minute
  after you stop.
- **Every game with cards in one list,** with its hours, drops and card set.
- **macOS and Linux.**

## Install

steamcards isn't released yet. To build it from source, with
[Rust](https://rustup.rs):

```sh
git clone https://github.com/joshgallantt/steamcards
cd steamcards
cargo run --release
```

## Getting started

Run `steamcards`. The first time, it walks you through signing in with the
Steam app and picking the games you want cards from first. Then leave it
running: it remembers everything for next time.

Press `?` at any time to see what the keys do.

To run without the dashboard, for a server or a log: `steamcards --headless`
(sign in with the dashboard first; it shows the QR code).

## Your data

Your sign-in and choices stay on your computer, in one file only you can
read (`~/Library/Application Support/steamcards` on macOS,
`~/.config/steamcards` on Linux). steamcards talks to nobody but Steam.
Signing out (press `a`) forgets the sign-in and ends it at Steam's end too.

## Good to know

steamcards is unofficial: Valve doesn't make, endorse or support it. As its
[licence](LICENSE) says, it comes with no warranty; use it at your own risk.

Steam won't drop cards for some games whatever plays them: family-shared
games, free-to-play games you haven't spent on, games marked private, and
games on limited accounts. steamcards moves on from a game that drops
nothing for 10 hours.

## Credits

How steamcards farms follows [ArchiSteamFarm](https://github.com/JustArchiNET/ArchiSteamFarm)
(Apache-2.0): see [NOTICE](NOTICE). Steam's messages are Valve's own, as
[SteamDatabase](https://github.com/SteamDatabase/Protobufs) publishes them.
The look is [streamdrops](https://github.com/joshgallantt/streamdrops)'.
