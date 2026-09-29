# Contributing to steamcards

## Running it from a clone

```sh
cargo run
```

To keep your real sign-in and choices out of the way while you work, point
steamcards at a file of its own:

```sh
STEAMCARDS_CONFIG=/tmp/steamcards-dev.json cargo run
```

`STEAMCARDS_DEBUG=1` writes debug lines (what's said to Steam, and what Steam
says back) to `debug.log` beside the config; any other value is a path to
write them to.

## Getting set up

```sh
cargo xtask setup
```

This installs the tools some checks need, at the versions CI uses (typos,
cargo-machete and cargo-deny), and turns on the git hook: from then on, every
commit first checks formatting, lints and tests. To skip the hook once, use
`git commit --no-verify`.

| Command | What it does |
| --- | --- |
| `cargo xtask ci` | Everything CI checks: formatting, spelling, lints, tests, docs, unused dependencies, and the dependencies' advisories, licences and sources. Run it before you push. |
| `cargo xtask lint`, `test`, `docs` or `deps` | One part of `ci`. |
| `cargo xtask pre-commit` | What the git hook runs: formatting, lints and tests. |
| `cargo xtask fix` | Formats the code and applies clippy's suggestions. |
| `cargo test -p <crate>` | One crate's tests, e.g. `cargo test -p farming`. |
| `cargo test -p terminal-ui previews -- --nocapture` | Draws every screen, in every state, into your terminal. |

## How the code is built

The folder tree is the dependency graph: each business concept under
`component/` has a `domain` crate (the entities, the rules and the
contracts), a `data` crate (what satisfies the contracts, over Steam or the
config file) and a `di` crate (what wires the two). Screens depend on domain
crates only. The [architecture reference](docs/architecture.md) has the whole
picture, and [the research](docs/research/steam-card-farming.md) says why
farming works the way it does.

## The rules the build enforces

- No `unsafe`, no dead code, `pub` only on what a crate exports.
- An exception to a lint says why, and fails once it's no longer needed:
  `#[expect(lint, reason = "…")]`, never `#[allow]`.
- No global state, no extension traits, no `Deref` to an inner type
  (`app/tests/language_rules.rs`).
- Only `app/src/settings.rs` reads the environment or the OS's directories.
- Only presentation prints.
- Dependencies come from crates.io, with licences that allow an MIT binary
  (`deny.toml`).

## Tests

Every change comes with its tests, at the tier that speaks its language: unit
tests beside the code, acceptance tests in the user's words in
`component/*/domain/tests/`, data tests against the stand-in Steam server in
`steam-api`'s `test_support`, and screen previews. No test talks to Steam.

## Being a good Steam citizen

steamcards asks Steam only for what the Steam client and the community site
ask for, at a polite pace. Stopping and restarting games to shake drops
loose is disputed (ASF calls it exploiting a Steam glitch; see the research
doc): it isn't in steamcards, and would need measuring first, and then only
as an opt-in.
