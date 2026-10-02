# Contributing to steamcards

Thanks for helping. This guide is the technical side of steamcards: building it yourself, running it on a server, how the code is built, the rules it follows, and how a change gets merged. If you only want to use steamcards, the [README](README.md) is the place.

- [Building from source](#building-from-source)
- [Running it: the technical details](#running-it-the-technical-details)
- [Setting up](#setting-up)
- [Everyday commands](#everyday-commands)
- [How the code is built](#how-the-code-is-built)
- [The rules the build enforces](#the-rules-the-build-enforces)
- [Tests](#tests)
- [Trying it against Steam](#trying-it-against-steam)
- [Pull requests](#pull-requests)
- [Cutting a release](#cutting-a-release)
- [Reporting bugs](#reporting-bugs)

---

## Building from source

Any OS, with Rust 1.89 or later and a C compiler. First set up your OS:

- **Windows:** install [Rust](https://www.rust-lang.org/tools/install), with `rustup-init.exe` or `winget install Rustlang.Rustup`. When rustup asks, let it install the **Visual Studio Build Tools** (the "Desktop development with C++" workload). If you skipped that, get them from [visualstudio.microsoft.com](https://visualstudio.microsoft.com/visual-cpp-build-tools/). On Windows on ARM, also install LLVM (`winget install LLVM.LLVM`): the `ring` crate needs Clang to build there.
- **macOS:** install the Xcode command-line tools, then [Rust](https://www.rust-lang.org/tools/install):
  ```sh
  xcode-select --install
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **Arch-based Linux:**
  ```sh
  sudo pacman -S --needed base-devel rustup xdg-utils
  rustup default stable
  ```
  Arch's own `rust` package works instead of `rustup`, since it's always recent.
- **Debian, Ubuntu and the like:**
  ```sh
  sudo apt install build-essential curl xdg-utils
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

Then build and install it:

```sh
cargo install --git https://github.com/joshgallantt/steamcards --locked steamcards
```

This puts `steamcards` in `~/.cargo/bin` (`%USERPROFILE%\.cargo\bin` on Windows), which rustup adds to your `PATH`. From a clone of the repository, `cargo install --path app --locked` does the same. To update it, run the same command with `--force`; to remove it, `cargo uninstall steamcards`.

## Running it: the technical details

### Without the dashboard

Once you've signed in (the dashboard shows the QR code the Steam app scans), steamcards can run with no screen at all, on a server or in a background terminal. It prints a line per event:

```sh
steamcards --headless                  # until you stop it
steamcards --headless --duration 3600  # stop after an hour (in seconds)
```

### Environment variables

| Variable | What it does |
| --- | --- |
| `STEAMCARDS_CONFIG` | Use a different config file, e.g. to keep your own sign-in out of the way while you work. |
| `STEAMCARDS_DEBUG` | Write a debug log: what's said to Steam, and what Steam says back. `1` puts it beside the config file, as `debug.log`; a path puts it there. In headless mode, debug lines go to the terminal's error output unless this says otherwise. |

### Where your data is

Your sign-in and choices are in one file, `config.json`, in a folder of steamcards' own. The market's prices are kept beside it, in `prices.json`, and `STEAMCARDS_DEBUG=1` puts the debug log there too. All three are readable by your user only:

| OS | Folder |
| --- | --- |
| macOS | `~/Library/Application Support/steamcards` |
| Linux | `$XDG_CONFIG_HOME/steamcards` when `XDG_CONFIG_HOME` is set to a full path, otherwise `~/.config/steamcards` |
| Windows | `%LOCALAPPDATA%\steamcards` |

That's [`dirs::config_local_dir()`](https://docs.rs/dirs/latest/dirs/fn.config_local_dir.html), from Rust's `dirs` crate, plus `steamcards`. On Windows it's this computer's own folder, not the roaming `%APPDATA%` that follows you to other PCs on a network: your sign-in is this computer's session with Steam, and two PCs sharing it would knock each other off. It's the only folder of yours the uninstall scripts delete: a config file you keep elsewhere with `STEAMCARDS_CONFIG` stays where it is.

One steamcards at a time uses a config file: two would sign on to Steam as the same session and knock each other off. While one runs, it holds `config.lock` beside the config file, and a second says so and quits. To run two, give the second a config file of its own with `STEAMCARDS_CONFIG`.

### Opening links on Linux

steamcards opens a game's card page in your browser through `$BROWSER` or the desktop's opener, such as `xdg-open` from `xdg-utils`. Most desktops have one. A text browser, like lynx, takes over the terminal until it quits.

### The install scripts

The one-line installers are [`install/install.sh`](install/install.sh) for macOS and Linux, and [`install/install.ps1`](install/install.ps1) for Windows. Each downloads the release for your system, checks it against the release's `SHA256SUMS`, and installs it without admin rights:

- on macOS and Linux, to `~/.local/bin`, or wherever steamcards already is;
- on Windows, to `%LOCALAPPDATA%\Programs\steamcards`, which it adds to your user `PATH`.

They won't overwrite a copy that Homebrew or cargo installed; they say how to update that one instead. Run again, they compare `steamcards --version` with the latest release, and only download when it's newer.

They read these variables, all optional:

| Variable | What it does |
| --- | --- |
| `STEAMCARDS_VERSION` | Install this release, e.g. `0.1.0`, instead of the latest. It's how to install a pre-release. |
| `STEAMCARDS_FORCE` | `1` reinstalls, even if that version is already installed. |
| `STEAMCARDS_INSTALL_DIR` | macOS and Linux only: install here instead of `~/.local/bin`. |
| `STEAMCARDS_RELEASES_URL` | Download from a mirror instead of the [releases page](https://github.com/joshgallantt/steamcards/releases). It needs the same layout: `<url>/latest` redirects to `<url>/tag/vX.Y.Z`, and each release's files are in `<url>/download/vX.Y.Z/`. |

For example:

```sh
curl -fsSL https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/install.sh | STEAMCARDS_VERSION=0.1.0 sh
```

Without the scripts, every release's archives are on the [releases page](https://github.com/joshgallantt/steamcards/releases/latest), one per platform: on Windows, `steamcards-x86_64-pc-windows-msvc.zip` holds `steamcards.exe`, to put wherever you like. The [security policy](.github/SECURITY.md#checking-a-download) says how to check one.

### The uninstall scripts

The one-line uninstallers, in the [README](README.md#your-data-and-uninstalling), are [`install/uninstall.sh`](install/uninstall.sh) for macOS and Linux, and [`install/uninstall.ps1`](install/uninstall.ps1) for Windows. They find steamcards as the install scripts do, in the install folder or else first on your `PATH`, and remove it the way it was installed:

- a copy from Homebrew with `brew uninstall steamcards`, using the `brew` beside it (a Mac can have two Homebrews), then `brew untap joshgallantt/steamcards` if that tap is there;
- a copy that cargo built, in `~/.cargo/bin` (or `$CARGO_HOME/bin`), with `cargo uninstall steamcards`, so that cargo forgets it too;
- any other copy by deleting it. On Windows, that's the whole `%LOCALAPPDATA%\Programs\steamcards` folder when steamcards is in it, or else just `steamcards.exe`.

If `brew` or `cargo` can't be found, they stop and say the command to run instead. Like the install scripts, they never use `sudo` or admin rights, so a copy in a folder only an administrator can change is left for you to delete. On Windows, they also take `%LOCALAPPDATA%\Programs\steamcards` off your user `PATH`, even if you deleted steamcards by hand, and leave every other entry as it was. They won't delete a `steamcards.exe` that's running.

Then they ask whether to delete your sign-in and choices too: the folder in [Where your data is](#where-your-data-is), and nothing else. When they can't ask (with no terminal, or in a PowerShell that can't prompt), they keep it and say where it is. `STEAMCARDS_DELETE_DATA` answers for you: `1` deletes it without asking, and `0` keeps it. For example:

```sh
curl -fsSL https://raw.githubusercontent.com/joshgallantt/steamcards/main/install/uninstall.sh | STEAMCARDS_DELETE_DATA=1 sh
```

In PowerShell, set it first: `$env:STEAMCARDS_DELETE_DATA = 1`. A copy you installed with `STEAMCARDS_INSTALL_DIR` is found as long as that folder is on your `PATH`.

---

## Setting up

1. Install rustup and your OS's build tools, as in [Building from source](#building-from-source). You don't choose a Rust version: [`rust-toolchain.toml`](rust-toolchain.toml) pins the one CI uses, and rustup installs it the first time you run `cargo` in the repository.
2. Clone the repository and, before anything else, set it up:
   ```sh
   git clone https://github.com/joshgallantt/steamcards
   cd steamcards
   cargo xtask setup
   ```
   This installs the tools some checks need, at the versions CI uses: [typos](https://github.com/crate-ci/typos), [cargo-machete](https://github.com/bnjbvr/cargo-machete) and [cargo-deny](https://github.com/EmbarkStudios/cargo-deny). `cargo install` builds them, which takes a few minutes the first time. Then it turns on the git hook: from then on, every commit first checks formatting, lints and tests. Git never runs a repository's own hooks until you opt in, and this is the opt-in. To skip the hook once, use `git commit --no-verify`. CI checks the same things on every pull request anyway.

   When a pull request changes a tool's version, run `cargo xtask setup` again. It only installs what's missing or out of date.
3. Run it with a separate config file, so your own sign-in and choices stay untouched:
   ```sh
   STEAMCARDS_CONFIG=/tmp/steamcards-dev.json cargo run
   ```
   In PowerShell: `$env:STEAMCARDS_CONFIG = "$env:TEMP\steamcards-dev.json"; cargo run`.

## Everyday commands

| Command | What it does |
| --- | --- |
| `cargo xtask setup` | Installs the tools the checks need, at the versions CI uses, and turns on the git hook. |
| `cargo xtask ci` | Everything CI checks: formatting, spelling, lints, dead code across crates, tests, docs, the README's screenshots, unused dependencies, and the dependencies' security advisories, licences and sources. Run it before you push. |
| `cargo xtask lint`, `test`, `docs` or `deps` | One part of `ci`, as CI runs it: formatting, spelling, lints and dead code; the tests; the docs and the README's screenshots; or the dependencies. |
| `cargo xtask pre-commit` | What the git hook runs: formatting, lints and tests. |
| `cargo xtask fix` | Formats the code and applies clippy's suggestions. |
| `cargo xtask hooks` | Turns on the git hook, without installing the tools. |
| `cargo test -p <crate>` | One crate's tests, e.g. `cargo test -p farming`. |
| `cargo test -p terminal-ui previews -- --nocapture` | Draws every screen, in every state, into your terminal. |
| `cargo xtask screenshots` | Redraws the README's images, in `docs/images/`, from the previews. |
| `cargo xtask protect` | Puts the rules for `main` and for tags on GitHub (maintainer only): see [Pull requests](#pull-requests). |

`cargo xtask` is a small Rust tool in [`xtask/`](xtask/src/main.rs), so it works the same everywhere. Three of its checks use a tool that doesn't come with Rust: spelling (typos), unused dependencies (cargo-machete) and the dependencies themselves (cargo-deny). Without the tool, that check is skipped locally, with a hint; CI always runs it. The tools' versions are set in one place, `TOOLS` in [`xtask/src/main.rs`](xtask/src/main.rs), and CI installs the same ones.

After changing the UI, run `cargo xtask screenshots` to redraw the README's images. They're drawn from the previews' made-up data (`readme_previews` in [`preview.rs`](ui/terminal-ui/src/app/preview.rs)), so they never show a real account; don't replace them with screenshots of a signed-in steamcards. With `PREVIEW_DUMP=<dir>`, the previews also write each screen's cells as JSON. The screens' design, and the rules that keep anything from being cut off, are in [docs/design/ui.md](docs/design/ui.md).

---

## How the code is built

The code is laid out the way Robert C. Martin's *Clean Architecture* describes, and every boundary is enforced by Cargo rather than by good intentions. This section follows **one keypress through every layer**. The full reference is [docs/architecture.md](docs/architecture.md).

### The one idea

> *"Source code dependencies must point only inward, toward higher-level policies."*
> — Robert C. Martin, *Clean Architecture* (2017), Chapter 22

```
Presentation ──▶ Domain ◀── Data
     └──────────────────────────▶ (never)
```

The domain is the rules: what gets farmed first, one game at a time or together, what a card is worth. It sits in the middle and depends on nothing. The screens depend on it. The Steam client depends on it. Nothing depends on them.

### Where everything lives

```
├── component/     Domain + Data + DI. One folder per business concept.
│   ├── account/       The one Steam account: signing in with a QR code, and out, and its wallet.
│   ├── game/          Your games with trading cards, their drops, and playing them.
│   ├── card/          Each game's cards, normal and foil: its sets, the copies you hold, new items, and what they sell for.
│   ├── preferences/   What you want farmed first.
│   ├── session/       This session: every card that dropped, and how long the rest should take.
│   ├── farming/       What to play, and how, and stepping aside for another device. Keeps nothing: no data crate.
│   └── money/         Amounts in a currency, as Steam counts and writes them.
├── library/       Infrastructure with no domain knowledge.
│   ├── steam-api/     The CM connection, QR sign-in, the pages several components read.
│   ├── config-file/   The one JSON file: the saved sign-in and preferences.
│   └── debug-log/     The opt-in debug log.
├── ui/            Presentation. Depends on domain crates, and farming-words.
│   ├── farming-words/ What the farmer says happened, in words, for both screens.
│   ├── terminal-ui/   View models and the ratatui dashboard.
│   └── headless/      --headless: events as plain text.
├── app/           The composition root: the one crate that names concrete types.
└── xtask/         Project tooling, run as `cargo xtask <task>`.
```

Each component is split into a `domain` crate (the rules and the contracts), a `data` crate (what satisfies the contracts) and a `di` crate (what wires the two). One that keeps nothing has no `data` crate: `farming` works through other components' use cases, and `session` is kept in memory. Each folder is its own crate, so the folder tree *is* the dependency graph. See [docs/architecture.md](docs/architecture.md#module-dependencies) for the whole graph.

### The walkthrough: you press `1` on a game

On the dashboard, you select a game and press `1`. You want its cards first. Here are the files that press reaches, in order:

```
      press 1
        │
        ▼
 ①  App::set_tier →                     ui/terminal-ui                presentation
     FarmingViewModel::set_tier
        │  calls a use case trait
        ▼
 ②  DefaultSetGameTierUseCase           component/preferences/domain  domain  ── the rule
        │  calls a repository trait
        ▼
 ③  PreferencesRepository               component/preferences/domain  domain  ── the contract
        ┆  ...is implemented by
        ▼
 ④  DefaultPreferencesRepository        component/preferences/data    data    ── the detail
        │  keeps them through a store
        ▼
     FilePreferencesStore
        │
        ▼
 ⑤  ConfigFile                          library/config-file           disk
```

The line at ③ is where it turns. The contract lives in the **domain**, and the thing that satisfies it lives in **data**. So the import arrow between them points *up* the page while the call goes *down*.

#### ① The screen holds only what it calls

**[`ui/terminal-ui/src/dashboard/farming_view_model.rs`](ui/terminal-ui/src/dashboard/farming_view_model.rs)**
```rust
pub struct FarmingViewModel {
    farm: Arc<dyn FarmCardsUseCase>,
    end_session: Arc<dyn EndSessionUseCase>,
    account: Arc<dyn GetAccountUseCase>,
    get_preferences: Arc<dyn GetPreferencesUseCase>,
    set_tier: Arc<dyn SetGameTierUseCase>,
    ...
```

Use cases, each one the view model actually calls. It holds no repository, no Steam connection and no config file. Its crate can't hold them either: [`ui/terminal-ui/Cargo.toml`](ui/terminal-ui/Cargo.toml) lists domain crates only, so `use card_data` doesn't resolve.

#### ② The rule lives in the domain, once

**[`component/preferences/domain/src/use_cases/preferences_use_cases.rs`](component/preferences/domain/src/use_cases/preferences_use_cases.rs)**
```rust
pub trait SetGameTierUseCase: Send + Sync {
    fn call(&self, app_id: AppId, tier: Tier) -> Result<(), PreferencesError>;
}
```

**[`component/preferences/domain/src/use_cases/impl/default_set_game_tier_use_case.rs`](component/preferences/domain/src/use_cases/impl/default_set_game_tier_use_case.rs)**
```rust
impl SetGameTierUseCase for DefaultSetGameTierUseCase {
    fn call(&self, app_id: AppId, tier: Tier) -> Result<(), PreferencesError> {
        let mut p = self.repo.preferences();
        p.priority_games.retain(|&g| g != app_id);
        p.skipped_games.retain(|&g| g != app_id);
        match tier {
            Tier::Priority(rank) => {
                let at = rank.saturating_sub(1).min(p.priority_games.len());
                p.priority_games.insert(at, app_id);
            }
            Tier::Indifferent => {}
            Tier::Skip => p.skipped_games.push(app_id),
        }
        self.repo.save(p).map_err(|_| PreferencesError::Unavailable)
    }
}
```

A use case is a trait named for what the user wants, and `DefaultSetGameTierUseCase` is the real one, over the repository. The screen calls it as `self.set_tier.call(app_id, tier)`, and a test hands it a double of its own: a `Stub…UseCase` that answers as it's told, or a `Spy…UseCase` that keeps what it's asked.

"A game is in exactly one tier" and "#1 bumps the others down" are business rules, so they live in the domain. A second screen that ranks games (the games pop-up does) calls this same use case and gets the same rules.

The failure is in the user's vocabulary, not the disk's:

```rust
pub enum PreferencesError {
    Unavailable,
}
```

To the user, a full disk and a failed rename are the same thing: *the change didn't stick*.

#### ③ The contract belongs to the domain

**[`component/preferences/domain/src/repository/preferences_repository.rs`](component/preferences/domain/src/repository/preferences_repository.rs)**
```rust
pub trait PreferencesRepository: Send + Sync {
    fn preferences(&self) -> Preferences;

    /// Errs when the preferences could not be kept, so a caller cannot report
    /// a change that did not happen. What `preferences` returns afterwards is
    /// what was kept.
    fn save(&self, preferences: Preferences) -> anyhow::Result<()>;
}
```

**The domain states what it needs, and the data layer is written to fit.** The doc comment is part of the contract. Any implementation that swallowed a write failure would still compile, but the user would be told their ranking changed when it didn't.

#### ④ The detail satisfies the contract

**[`DefaultPreferencesRepository`](component/preferences/data/src/default_preferences_repository.rs)** keeps the preferences through a `PreferencesStore`. The store beside that trait, **[`FilePreferencesStore`](component/preferences/data/src/preferences_store.rs)**, maps `Preferences` onto the file's shape, a `PreferencesDto`, and **[`library/config-file/src/config_file.rs`](library/config-file/src/config_file.rs)** writes it among the file's other fields:

```rust
pub fn write<T: Serialize>(&self, part: &T) -> anyhow::Result<()> {
    let Value::Object(part) = serde_json::to_value(part)? else {
        anyhow::bail!("only named fields can be written to the config file");
    };
    let mut fields = self.fields.lock().unwrap();
    let mut next = fields.clone();
    next.extend(part);
    write_whole(&self.path, &serde_json::to_vec_pretty(&next)?)?;
    *fields = next;
    Ok(())
}
```

It writes first, then keeps. If it kept first, a failed write would leave memory holding a ranking the file doesn't have.

Here's the structural claim, which you can check yourself:

```
component/preferences/
├── domain/   ← crate `preferences`       (the rule, the contract)
├── data/     ← crate `preferences-data`  (the implementation)
└── di/       ← crate `preferences-di`    (the wiring)
```

[`component/preferences/domain/Cargo.toml`](component/preferences/domain/Cargo.toml) doesn't list `preferences-data`. **Import the data layer from the domain and the workspace stops building.** That's the difference between an architecture and a diagram of one.

#### ⑤ …and back out

If the write succeeded, the dashboard flashes *"Hades is now priority #1."* If it failed:

**[`ui/terminal-ui/src/app/mod.rs`](ui/terminal-ui/src/app/mod.rs)**
```rust
if let Err(err) = self.farming.set_tier(e.game.app_id, tier) {
    return self.didnt_stick(err);
}
```

Instead of confirming a change that didn't happen, it says *"That didn't stick — preferences couldn't be saved. Try again?"*

The farmer never hears about the keypress. Every tick, it calls `GetPreferencesUseCase` to see what the user wants. [`component/farming/domain/src/use_cases/impl/farmer.rs`](component/farming/domain/src/use_cases/impl/farmer.rs) notices the preferences changed, plans again, and switches to Hades if the new plan says so. Farming depends on the preferences **use case**, never on its storage.

### What that buys

- **Dependency inversion** (③): `GameRepository`, `PlayingRepository`, `CardRepository`, `CardPriceRepository`, `AccountRepository` and `PreferencesRepository` are all declared in domain crates and implemented in data crates. Imports run Data → Domain while calls run Domain → Data.
- **Single responsibility**: Steam's CM protocol, and the page markup several components read, change for Valve's reasons and live in `library/steam-api`; a page only one component reads is read in its data crate, as the badge pages are in `game-data`. The rules for what to farm change for the user's reasons and live in `component/farming/domain`.
- **Interface segregation** (①): one trait per use case, so the games pop-up holds the preference use cases it needs and the account pop-up holds the account ones. Neither sees the farmer.
- **Liskov substitution**: the paused-time tests drive the real `DefaultFarmCardsUseCase` over a fake Steam account, and the farmer can't tell the difference.

---

## The rules the build enforces

None of these are conventions to remember. Break one and the build, a test or CI fails.

**The dependency rule.** Each layer may only depend on the layers it's allowed to:

| Layer | Crates | May depend on |
| --- | --- | --- |
| Domain | `money`, `account`, `game`, `card`, `session`, `preferences`, `farming` | Domain |
| Data | `*-data` | Domain, Library |
| DI | `*-di` | Domain, Data, Library |
| Library | `config-file`, `debug-log`, `steam-api` | Library |
| Presentation | `farming-words`, `terminal-ui`, `headless` | Domain, Presentation |
| App | `steamcards` | Domain, DI, Library, Presentation |
| Tooling | `xtask` | nothing in the workspace |

The compiler enforces it, because a crate can only `use` what its `Cargo.toml` lists. [`app/tests/dependency_rule.rs`](app/tests/dependency_rule.rs) reads every manifest and fails on any arrow the table doesn't allow, on a domain crate using a component it shouldn't (`card` never uses `farming`), and on production code enabling a `test-support` feature.

**Rust features this codebase doesn't use:** extension traits, global `static` state, `Deref` as inheritance, reading the environment outside [`app/src/settings.rs`](app/src/settings.rs), glob imports, printing outside presentation, and `unsafe`. [`app/tests/language_rules.rs`](app/tests/language_rules.rs), the workspace lints and [`clippy.toml`](clippy.toml) check them. [docs/architecture.md](docs/architecture.md#rust-features-this-codebase-doesnt-use) says why each one is out, and what to do instead.

**The domain says what happened; the screens choose the words.** The farmer reports `FarmingEvent::Dropped { game, count, left }`, never *"A card dropped for Hades — 1 to go"*: [`ui/farming-words`](ui/farming-words) words it for both the dashboard and `--headless`, and the dashboard's `price_words` words the market's `PriceEvent`s. A domain test checks the event; the sentence is tested where it's written.

**Use cases are traits.** Each is named for what the user wants and has one method, `call`, like `pub trait SignOutUseCase: Send + Sync { fn call(&self) -> Result<(), SignOutError>; }`. All of a component's are declared in `use_cases/<name>_use_cases.rs`, and each is done by a `Default…UseCase` in a file of its own under `use_cases/impl/` (`impl` is a keyword, so the module is `r#impl`). Callers hold `Arc<dyn SignOutUseCase>`. Its test doubles are types too, a file each: `StubGetAccountUseCase` in `test_support/stubs/`, `SpySignOutUseCase` in `test_support/spies/`.

**Lints are errors, not warnings.** They're set once for the whole workspace in the root [`Cargo.toml`](Cargo.toml):
- no dead code: an unused function, field, import, variable or ignored `#[must_use]` result fails the build;
- `unreachable_pub`: an item is `pub` only if its crate exports it;
- clippy's default set, plus no glob imports, no printing outside presentation, and no `dbg!`, `todo!` or `unimplemented!`;
- broken links in doc comments.

**Dead code across crates fails too.** The compiler judges a crate at a time, and counts what a crate exports as used, even when nothing else calls it. No crate here is published, so `cargo xtask dead-code`, part of `lint`, fails on a library's public function that nothing else names, or that only tests do. Test support goes behind the crate's `test-support` feature, like `SteamClient::with_ask_again_after`; what nothing uses goes.

**Exceptions say why.** When a rule really doesn't fit, write `#[expect(lint, reason = "…")]`. `#[allow]` is itself denied. An `expect` also fails the checks once it's no longer needed, so none are left behind.

**Dependencies are checked too.** Every crate comes from crates.io, under a licence that works in an MIT binary, with no known security advisory, in a version its author hasn't pulled. [`deny.toml`](deny.toml) lists the licences allowed, the crates that aren't and why, and any advisory accepted for now, with its reason. A new dependency that breaks one of these fails the checks until it's been looked at.

**Everything is pinned, and updated on purpose.** `Cargo.lock` holds every crate's exact version and checksum, and CI and releases build with `--locked`. Every GitHub Action is pinned to a commit, and the compiler and CI's tools to versions. Nothing changes until a pull request changes it. [Dependabot](.github/dependabot.yml) opens those pull requests once a week: one for compatible Cargo updates, one for GitHub Actions, and one for each new Rust. A release has to be a week old before it's proposed, and security fixes come straight away. Merge them when the tests pass, after a look at what changes. Breaking upgrades aren't proposed: make those by hand, now and then.

**Spelling** is checked in code, comments and docs. When typos flags a word that's right, add it to [`typos.toml`](typos.toml), as narrowly as you can, with a comment saying why.

**Formatting** is plain `rustfmt`, with no configuration. `cargo xtask fix` applies it.

---

## Tests

| Tier | Where | Speaks |
| --- | --- | --- |
| Unit | `#[cfg(test)]` next to the code, and `tests/` in each domain crate | the system's terms, and the use cases' rules over the component's fakes |
| Acceptance | `tests/` in each DI crate, with the driver in `tests/support/` | the user's terms, through the component as the app wires it: `player.signs_in()`, `player.comes_back()` |
| Paused time | `tests/` in the farming and card domain crates | the user's terms, over hours of play: `player.starts_farming()`, `player.reads(dropped)` |
| End to end | `tests/` in the data and library crates | real code against local stand-ins for Steam: a CM server over a real WebSocket (`steam-api`'s `test_support`), and [wiremock](https://crates.io/crates/wiremock) for steamcommunity.com |
| Screens | [`ui/terminal-ui/src/app/preview.rs`](ui/terminal-ui/src/app/preview.rs) | what's on screen, and that nothing is cut off at any size |
| Architecture | [`app/tests/`](app/tests/) | the rules above |

An acceptance test reads like the user's day, and runs the component as the app wires it, over its real data layer and a real config file, against a stand-in for Steam:

**[`component/account/di/tests/signing_in.rs`](component/account/di/tests/signing_in.rs)**
```rust
#[tokio::test]
async fn a_sign_in_thats_kept_is_there_when_steamcards_starts_again() {
    let player = Player::new("kept").await;
    player.steam.qr_goes(approved());
    player.signs_in().await.1.unwrap();

    let player = player.comes_back();

    assert_eq!(player.sees(), signed_in(false));
}
```

A paused-time test does the same over hours of play, which a real connection couldn't run on:

**[`component/farming/domain/tests/farming_cards.rs`](component/farming/domain/tests/farming_cards.rs)**
```rust
#[tokio::test(start_paused = true)]
async fn a_game_with_three_hours_is_farmed_alone_until_every_card_drops() {
    let mut player = Player::new();
    player.steam.drops_only_alone();
    player.steam.add_game(620, 5.0, 3, Some(30 * MINUTE));
    player.steam.add_game(440, 1.0, 2, Some(30 * MINUTE));
    player.starts_farming();

    assert_eq!(
        player.reads(playing).await,
        FarmingCards {
            game: "Game 620".into(),
            cards_left: 3
        }
    );
    ...
}
```

This runs the real farmer through hours of drops in milliseconds, on paused time, with no network. It checks what the farmer says happened; how the log puts it is `farming-words`' to test.

- **Doubles** come from the domain crates' `test-support` features, which production builds never enable: a file each, named for their kind (`FakeGameRepository`, `StubGetAccountUseCase`, `SpySignOutUseCase`), and builders like `game()`.
- **No test touches the real Steam.** End-to-end tests point `steam-api` at a local stand-in through its `Endpoints`.
- **Use made-up account names, Steam IDs and tokens** in tests, fixtures and docs, never real ones.
- **A test should fail when the code is wrong.** If you add one, break the code on purpose once and watch it fail.

**The install and uninstall scripts** have tests of their own, in [`install/tests/`](install/tests/). Each scenario runs one of the scripts with a home, install folder and data folder of its own, so your own steamcards and sign-in are never touched: the install scripts against a local stand-in for GitHub Releases, and the uninstall scripts with stand-ins for Homebrew and cargo. `uninstall.sh` runs with no terminal, as on CI, or with one of its own that answers its question ([`terminal.py`](install/tests/terminal.py)). They need Python 3. `sh install/tests/run.sh` tests `install.sh` and `uninstall.sh` under sh, dash and BusyBox, whichever you have. `pwsh -NoProfile -File install/tests/run.ps1` tests `install.ps1` and `uninstall.ps1`; on macOS and Linux it skips the scenarios that need Windows, and on Windows it puts your user `PATH` back as it was after each one. CI runs them on Linux, macOS and Windows, under Windows PowerShell 5.1 and PowerShell 7, whenever the scripts change.

---

## Trying it against Steam

**Only what Steam's own client does.** steamcards asks Steam only for what the Steam client and the community site ask for, at a polite pace, and farms one account. Never add anything that fakes or gets around Steam's security, or that games the drop timer: stopping and restarting games to shake drops loose is disputed (ArchiSteamFarm calls it exploiting a Steam glitch), so it isn't in steamcards. A pull request that does otherwise won't be merged.

Much of what steamcards calls is undocumented and changes without notice. How farming works, cross-checked against the other farmers, is in [docs/research/steam-card-farming.md](docs/research/steam-card-farming.md); where prices, drops and the wallet come from is in [docs/research/market-and-session.md](docs/research/market-and-session.md). If you find that something changed, add to them.

When you try a change for real:

- **Always use a separate config file** (`STEAMCARDS_CONFIG`), never the one you use day to day.
- **Turn on the debug log** with `STEAMCARDS_DEBUG=1`, or give it a path.
- **Don't share tokens, account names or Steam IDs.** Keep them out of issues, pull requests, commits and test fixtures. Check a debug log before you attach it.
- **Be gentle.** The market's pages are rate-limited: steamcards asks one thing at a time, seconds apart, and waits when Steam says to. Don't make it faster.

---

## Pull requests

1. **Branch from `main`**, and keep each pull request to one change.
2. **Test the change.** New behaviour gets a test; a bug fix gets a test that failed before the fix.
3. **Update the docs.** The README covers anything a user sees; this guide and [docs/architecture.md](docs/architecture.md) cover how the code is built.
4. **Title it for the people who use steamcards:** what's different, not how, like "Show foils' prices in a game's details". The next release's notes list each merged pull request by its title.
5. **Run `cargo xtask ci`.** It's what CI runs.
6. **Open the pull request.** The template's checklist matches these steps.

CI is one workflow, **Tests** ([`tests.yml`](.github/workflows/tests.yml)), run on every pull request and every push to `main`, with a job for each check:
- formatting, spelling, lints and dead code (`cargo xtask lint`), the docs and the README's screenshots (`cargo xtask docs`), and the dependencies (`cargo xtask deps`), once, on Linux;
- `cargo check` on the oldest supported Rust, the `rust-version` in [`Cargo.toml`](Cargo.toml), and actionlint and zizmor on the workflows themselves;
- the tests (`cargo xtask test`) on every system steamcards supports: Linux, macOS, Windows and an Arch Linux container;
- when the install scripts change, ShellCheck and PSScriptAnalyzer on them, and their tests on Linux, macOS and Windows.

Its last job, "Tests passed", passes once every other job has passed or was skipped as not needed. It's the one check pull requests need.

Two more workflows run less often. [`release.yml`](.github/workflows/release.yml) publishes a release (see [Cutting a release](#cutting-a-release)). When a release changes the Homebrew formula, [`homebrew.yml`](.github/workflows/homebrew.yml) installs it and checks it runs.

**Merging and releasing.** Two GitHub rulesets, kept in [`.github/rulesets/`](.github/rulesets/), protect the repository:
- [`main.json`](.github/rulesets/main.json): changes reach `main` through pull requests, and the tests have to pass. Only the maintainer can merge them, or push to `main` at all, and `main` can't be force-pushed or deleted.
- [`tags.json`](.github/rulesets/tags.json): only the maintainer can create, move or delete a tag. Pushing a `v*` tag is what publishes a release, so only the maintainer can publish one.

The maintainer can go around the rules when needed: releases push straight to `main`. `cargo xtask protect` puts both rulesets on GitHub, or brings them back in line with the files. GitHub enforces rules on a public repository, or on a private one with GitHub Pro.

A pull request merges once the tests pass and the maintainer has reviewed it.

**Commit messages:** a short summary in the imperative ("Show foils' prices in a game's details"), then a blank line and why, if it isn't obvious.

---

## Cutting a release

One command does it, and asks once before anything leaves your machine:

```sh
cargo xtask release minor    # or patch, major, or a version like 0.3.0-rc.1
```

It needs the [GitHub CLI](https://cli.github.com), signed in (`gh auth login`). It stops if nothing has changed since the last release.

What it does:

1. **Checks** you're on `main`, with nothing uncommitted, level with GitHub.
2. **Prepares the release on your machine.** It works out the version and runs `cargo xtask ci` on the code as it is, so a failed check leaves nothing to undo. Then it bumps `Cargo.toml` and `Cargo.lock`, commits "Release vX.Y.Z" and tags it. The commit skips the pre-commit hook, whose checks have just passed; GitHub tests it before the release builds.
3. **Asks, then pushes** `main` and the tag together. `--yes` skips the question.
4. **Follows the release build** ([`release.yml`](.github/workflows/release.yml)) to the end. The build:
   - waits for the tests to pass on the tagged commit, which pushing it to `main` started;
   - builds macOS (Apple silicon and Intel), Linux (x86_64 and arm64, static) and Windows (x86_64), and checks each build runs and reports the tag's version;
   - publishes them as a GitHub release, with `SHA256SUMS` and, for a public repository, [build-provenance attestations](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations). Its notes say how to install it, then GitHub lists the pull requests merged since the last release, grouped as [`.github/release.yml`](.github/release.yml) says. Edit them on the release's page to add anything else;
   - for a public repository, installs the release with the install scripts, the way users do on each platform, checks it runs, and uninstalls it again.
5. **Points the [Homebrew formula](Formula/steamcards.rb) at the release**, and pushes that to `main`. [`homebrew.yml`](.github/workflows/homebrew.yml) then installs it with Homebrew and checks it runs.

It needs no secrets, tokens or repository settings. The workflows never push to `main`; only you do, which is what lets `main` stay protected.

**If something stops it** (a failed check, Ctrl-C, a closed laptop), run the same command again with the same version: it carries on from where it stopped. Once the tag is pushed, the release itself finishes on GitHub whether or not your machine is watching; running the command again afterwards points Homebrew at it. If the build fails before publishing, nothing is published: fix it on `main` and release the next version, or move the tag to the fix (`git push origin :refs/tags/vX.Y.Z && git tag -d vX.Y.Z`) and run the command again.

**A pre-release** is a version with a hyphen, like `0.3.0-rc.1`. It's marked as one on GitHub, and Homebrew and the install scripts' "latest" skip it. People install it by name, with `STEAMCARDS_VERSION`.

---

## Reporting bugs

Open an issue using the bug report template. The useful things to include are your OS, your terminal, what happened and what you expected. A debug log helps (`STEAMCARDS_DEBUG=1`); read it first and remove anything private: account names, Steam IDs and tokens.

---

## References

- Robert C. Martin, *Clean Architecture: A Craftsman's Guide to Software Structure and Design* (2017)
- Eric Evans, *Domain-Driven Design* (2003)
- Mark Seemann & Steven van Deursen, *Dependency Injection: Principles, Practices, and Patterns* (2019)
- The layout follows [Real Clean Architecture in iOS](https://github.com/joshgallantt/Real-Clean-Architecture-in-iOS-Example), translated from Swift packages to Cargo crates.
