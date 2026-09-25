# cosmic-ext-applet-timer

Generic countdown timer applet for the COSMIC panel (libcosmic / iced, Rust).
Design, risks and milestones live in `PLANNING.md`; read it first.

## Working method (non-negotiable)

- **The user writes the code.** This project is also how they learn Rust
  (never written it before). The assistant guides step by step, says what to
  write and where, explains each new construct when it appears, reviews, and
  unblocks. Do not write the implementation yourself unless the user
  explicitly asks for a specific snippet.
- **Explain before code, then code in pieces.** For every step: first say
  what we are about to build and why, then walk through the code a few
  lines at a time, explaining each piece before showing it. Never dump a
  whole file and explain it afterwards.
- Propose before doing: present the plan for a step, wait for the OK.
- Never commit without asking. Conventional Commits (`feat:`, `fix:`, `docs:`,
  `chore:`, `refactor:`, `test:`), minimal: a single subject line unless the
  change needs explaining.
- Code, comments, docs and commit messages in English. The user may write in
  Italian.
- Follow the milestone order in `PLANNING.md`; do not pull later features
  forward.

## Identity

- Crate / binary / directory: `cosmic-ext-applet-timer`
- App id: `io.github.angelinuxx.CosmicExtAppletTimer` (used for .desktop,
  icon name, cosmic-config directory)
- Licence: MPL-2.0

## Toolchain

- Rust stable, pinned in `rust-toolchain.toml` (1.98 at project start).
- `edition = "2024"`.
- `libcosmic` from `https://github.com/pop-os/libcosmic` with an explicit
  `rev` in `Cargo.toml`. Bumping the rev is a dedicated task, never a side
  effect of other work. Features: `applet`, `applet-token`, `dbus-config`,
  `multi-window`, `tokio`, `wayland`, `winit`.
- Other deps: `tokio` (time, process), `i18n-embed` + `i18n-embed-fl` +
  `rust-embed` (Fluent), `notify-rust` (feature `z`), `serde`.
- Target machine: Pop!_OS 24.04, COSMIC packages must be current
  (`cosmic-panel` / `cosmic-applets` 1.9.x or later).
- `just` is the task runner.

## Commands

```
just build-debug          # cargo build
just build-release        # cargo build --release
just check                # cargo clippy --all-features -- -W clippy::pedantic
cargo test                # unit tests (timer.rs)
just dev-install          # symlink release binary + .desktop + icon into ~/.local
just dev-reload           # rebuild + pkill cosmic-panel (session restarts it)
just install-user         # copy (not symlink) into ~/.local
just uninstall-user
```

First-time setup after `dev-install`: Settings -> Desktop -> Panel ->
Configure applets -> add "Timer".

Logs: `journalctl --user -f` or run `target/release/cosmic-ext-applet-timer`
from a terminal (it will attach to the running panel).

If the panel does not come back after `dev-reload`: `cosmic-panel &`.

## Layout

```
src/main.rs      entry point: i18n init + cosmic::applet::run
src/app.rs       AppModel, impl cosmic::Application, Message enum
src/timer.rs     pure timer state machine + unit tests, no UI, takes `now` as a param
src/config.rs    settings via cosmic-config (CosmicConfigEntry)
src/state.rs     persisted runtime state (running timers)
src/notify.rs    desktop notification + optional sound (pw-play)
src/i18n.rs      Fluent loader and fl! macro
i18n/<lang>/cosmic_ext_applet_timer.ftl
resources/       app.desktop, app.metainfo.xml, icon.svg
```

## Conventions

- All state changes happen in `update`; `view` / `view_window` never mutate.
- Timer logic never reads the clock itself: pass `SystemTime::now()` in from
  `update` so tests can supply fake times.
- Colours come from the active COSMIC theme, never hard-coded RGB.
- Panel text goes through `core.applet.text()`; popup content through
  `core.applet.popup_container()`.
- Save persisted state on transitions, never on tick.
- Every user-visible string goes through `fl!`.
- Reference implementations (read-only, clone into the scratchpad when
  needed): `cosmic-utils/cosmic-ext-applet-pomodoro`,
  `pop-os/cosmic-applets` (`cosmic-applet-time`).
