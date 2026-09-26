# cosmic-ext-applet-timer: planning

A generic countdown timer applet for the COSMIC panel (System76, libcosmic/iced,
Rust). Not a Pomodoro: a plain "set a duration, start, get notified" timer, with
multiple labelled timers as a later increment.

This document is the agreed design and milestone plan (2026-09-25). `CLAUDE.md`
holds the project conventions and commands.

## Goals and scope

Must have (MVP = M0..M2):

- Set a duration: quick presets (5 / 10 / 15 min) plus manual minutes/seconds.
- Start, pause, resume, reset from the applet popup.
- Remaining time visible in the panel while running.
- System notification on expiry.
- An "expired, waiting for acknowledgement" state visible in the panel until
  the user clicks it.

Later increments:

- Persist running timers and settings across applet/panel restarts (M3).
- Several timers in parallel, each with a label (M4).
- Sound on expiry via an external player, Italian translation, vertical panel
  layout, packaging polish (M5).

Out of scope: Pomodoro work/break cycles, blinking panel text, an integrated
audio stack (rodio and friends).

## Learning context

The author is learning Rust with this project (no prior Rust written). The
author writes the code; the assistant guides, explains constructs as they
appear, and reviews. Milestones are deliberately small for this reason.

## Reference material

Studied before designing:

- `cosmic-utils/cosmic-ext-applet-pomodoro`: minimal community applet, the
  structural template for this project (layout, justfile, i18n, .desktop).
- `pop-os/cosmic-applets` (`cosmic-applet-time`): official applet, reference for
  panel text (`core.applet.text()`), horizontal/vertical layouts and a tick
  subscription that survives suspend.
- `ommstock/cosmic-applet-pomodoro`: uses a `com.system76.*` app id, which is
  wrong for third-party projects. Not followed.

### How a COSMIC applet is structured

- **Entry point** (`main.rs`): init Fluent localisation, then
  `cosmic::applet::run::<AppModel>(())`. The "main window" is a Wayland
  layer-shell surface hosted by the panel.
- **`cosmic::Application` trait** on `AppModel`: `APP_ID`, `init`, `view`
  (panel button), `view_window` (popup content inside
  `core.applet.popup_container`), `update` (the only place state changes),
  `subscription` (async event sources, declared from current state).
- **Popup**: on `TogglePopup`, create a `window::Id`, build settings with
  `core.applet.get_popup_settings`, return `get_popup(settings)`;
  `on_close_requested` clears `self.popup`. `X-CosmicHoverPopup=Auto` in the
  .desktop enables hover opening.
- **Tick**: a tokio stream in `subscription()` sends `Message::Tick` every
  second, only while something is running. iced starts/stops it by comparing
  the declared subscriptions after each update.
- **cosmic-config**: `#[derive(CosmicConfigEntry)]` + `#[version = N]`; one RON
  file per field under `~/.config/cosmic/<APP_ID>/vN/`. `watch_config` in the
  subscription reports external changes.
- **i18n**: `i18n.toml` -> `i18n/<lang>/<crate>.ftl`, embedded with
  `rust-embed`, read via the `fl!` macro.
- **Panel registration**: a .desktop file with `X-CosmicApplet=true` and
  `NoDisplay=true` in `~/.local/share/applications` (or `/usr/share`). Added
  from Settings -> Desktop -> Panel -> Configure applets. The panel does not
  hot-reload applets: rebuild then `pkill cosmic-panel`.

## Design

### Timer engine: absolute deadline

Chosen over a tick-decremented counter (drifts, breaks across suspend, cannot
be persisted) and over a dedicated async task (more moving parts, no benefit).

When a timer starts we store `ends_at: SystemTime`; remaining time is
`ends_at - now`, recomputed on every tick. The tick only triggers a redraw.
`SystemTime` (not `Instant`) because it keeps running during suspend and is
serialisable.

### Data model (`src/timer.rs`, no UI code)

```rust
pub enum TimerState {
    Idle,
    Running { ends_at: SystemTime },
    Paused  { remaining: Duration },
    Expired,                      // waiting for acknowledgement
}

pub struct Timer {
    pub id: u32,                  // needed for multi-timer messages
    pub label: String,            // empty in the MVP
    pub duration: Duration,       // configured length, for reset and progress
    pub state: TimerState,
}
```

Methods: `start(now)`, `pause(now)`, `resume(now)`, `reset()`,
`remaining(now) -> Duration`, `progress(now) -> f32`,
`tick(now) -> Option<Event>` (moves Running -> Expired when `now >= ends_at`
and returns `Some(Event::Expired)` so `update` knows when to notify).

Every method takes `now` as a parameter instead of reading the clock, so the
module is unit-testable without waiting.

`AppModel` holds a single `timer: Timer` until M3. Moving to `Vec<Timer>` is
the subject of M4 (decided 2026-09-25: carrying a one-element list through
three milestones adds noise without teaching anything).

`acknowledge()` is not a separate method: confirming an expired timer is a
`reset()` from the UI's point of view.

### Tick subscription

Present only while the timer is `Running`. `iced::time::every(1s)` mapped to
`Message::Tick` (simpler than a hand-written stream; revisit only if drift
becomes visible). `update` calls `tick(now)` and acts on the returned event.

### Panel (`view`)

- Idle: symbolic icon only.
- Running / Paused: icon + `core.applet.text()` with `mm:ss` under one hour,
  `h:mm` above; dimmed while paused. Vertical panel: icon only.
- Expired: `00:00` on a background using the theme accent colour (from
  `cosmic::theme::active()`, never hard-coded RGB). Left click acknowledges
  and returns to Idle.
- Multi-timer (M4): show the timer closest to expiry, plus a `+N` badge when
  others are running. Any Expired timer wins.
- Left click opens the popup (convention of the official applets).

### Popup (`view_window`)

Top to bottom: big remaining time (`title1`), progress bar, control row
(Start / Pause / Resume / Reset depending on state), divider, preset row
(5 / 10 / 15 as pill buttons), manual entry with two spin buttons (minutes,
seconds). In M4 each timer becomes a card with a label field, plus an
"Add timer" button.

### Notification and sound (`src/notify.rs`)

On the Running -> Expired transition `update` returns a `Task` that sends a
desktop notification through `notify-rust` (feature `z`, i.e. zbus, to share
the D-Bus stack with libcosmic). Title "Timer finished", body with label and
duration.

Sound (M5): spawn `pw-play` via `tokio::process::Command` on
`/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga`, behind a
settings toggle. No audio crate.

### Persistence (`src/config.rs`, `src/state.rs`)

- Settings (presets, sound on/off): `Config` with `CosmicConfigEntry`, stored
  under `~/.config/cosmic/<APP_ID>/v1/`, watched with `watch_config`.
- Runtime state (timers with `ends_at`): `State` via cosmic-config's state
  constructor (expected `Config::new_state`, writes under `~/.local/state/`).
  Saved on every state transition, never on tick. Reloaded at startup; a
  `Running` timer whose `ends_at` is in the past becomes `Expired` on the
  first tick and notifies immediately.
- Fallback if the state API is not available in the pinned libcosmic rev: a
  RON/JSON file under `~/.local/state/` written with `serde`.

### Project layout

```
cosmic-ext-applet-timer/
├── Cargo.toml              libcosmic from git, explicit rev
├── rust-toolchain.toml     pinned stable
├── justfile                build, install-user, dev-install, dev-reload, check
├── i18n.toml
├── i18n/en/cosmic_ext_applet_timer.ftl    (it/ later)
├── resources/              app.desktop, app.metainfo.xml, icon.svg
├── src/
│   ├── main.rs             entry point
│   ├── app.rs              AppModel, Application impl, Message enum
│   ├── timer.rs            pure timer logic + unit tests
│   ├── config.rs           settings (cosmic-config)
│   ├── state.rs            persisted runtime state
│   ├── notify.rs           notification + sound
│   └── i18n.rs
├── PLANNING.md
└── CLAUDE.md
```

Identity: crate `cosmic-ext-applet-timer`, app id
`io.github.angelinuxx.CosmicExtAppletTimer`, licence MPL-2.0.

### Testing

`timer.rs` has pure unit tests: start, pause/resume with correct remaining
time, expiry, `ends_at` in the past on reload. Everything else is verified by
hand in the panel; each milestone ends with `just dev-install` and a manual
checklist.

## Risks

1. **libcosmic has no usable releases.** Everyone pins git; the API changes
   between commits. Pin an explicit `rev` in `Cargo.toml`, chosen from what
   `pop-os/cosmic-applets` uses for the installed panel version. Bumping the
   rev is its own task, never a side effect.
2. **Installed panel is behind.** cosmic-panel 1.0.x installed, 1.9.0 pending.
   Upgrade before M0 so the pinned rev matches the running panel.
3. **Build time and disk.** First libcosmic build takes minutes and several GB
   in `target/`. Consider `CARGO_TARGET_DIR` on a faster disk if ExtraDrive is
   slow.
4. **System build deps.** libwayland-dev, libxkbcommon-dev, pkg-config,
   libssl-dev, libfontconfig-dev, libfreetype-dev, libexpat1-dev. All present
   on the dev machine as of 2026-09-24.
5. **Reloading during development.** `just dev-reload` = rebuild +
   `pkill cosmic-panel` (cosmic-session restarts it). Logs via
   `journalctl --user -f` or by running the binary from a terminal.
6. **State persistence API** in cosmic-config must be verified in the pinned
   rev at M3 (see fallback above).
7. **Notifications.** zbus version mismatch between notify-rust and libcosmic
   only costs compile time. If nothing shows up, check COSMIC's Do Not Disturb
   before the code.
8. **Toolchain.** `edition = "2024"` and async closures need Rust >= 1.85;
   `rust-toolchain.toml` pins the version.
9. **Learning curve.** The borrow checker will slow early milestones more than
   libcosmic will. Milestones are small on purpose.

## Milestones

Each milestone ends with something installed and clickable in the panel, and a
commit.

### M0: skeleton in the panel

- Upgrade COSMIC packages; pick and pin the libcosmic rev.
- `Cargo.toml`, `rust-toolchain.toml`, `main.rs`, `app.rs` with an `AppModel`
  that shows an icon and opens a popup containing the text "Timer".
- `justfile`, `resources/app.desktop`, icon, i18n with one string.
- `just dev-install`, add the applet to the panel.

Learn: cargo, the `Application` trait, `view` / `view_window` / `update`, the
popup cycle, how an applet is installed.

### M1: one running timer

- `timer.rs` with `TimerState` and methods, tests first (`cargo test`).
- Fixed 5 minute duration. Popup with big time and Start / Pause / Resume /
  Reset. Tick subscription. Remaining time in the panel.

Learn: enums with data, `match`, `Option`, `SystemTime` / `Duration`, unit
tests, subscriptions, `&` / `&mut` in practice.

### M2: chosen duration, expiry, notification (end of MVP)

- Presets 5 / 10 / 15 and minutes/seconds spin buttons.
- `Expired` state: `00:00` on accent background in the panel, click to
  acknowledge.
- Desktop notification via `notify-rust`. Progress bar.

Learn: async `Task`s, D-Bus from the application side, COSMIC theming, more
widgets.

### M3: persistence

- `config.rs` with presets and settings via cosmic-config.
- `state.rs` with running timers. Timer survives `pkill cosmic-panel` and
  logout; if it expired while the panel was down, notify on restart.

Learn: `serde`, custom derives, `watch_config`, file I/O through a library.

### M4: multiple labelled timers

- `Vec<Timer>` in the UI: one card per timer, label field, Add / Remove.
- Panel shows the soonest timer plus `+N`.

Learn: `Vec`, iterators, ownership inside collections, messages carrying ids.

### M5: polish

- Sound via `pw-play` with a settings toggle.
- Italian translation. Vertical panel layout.
- Complete `metainfo.xml`, README, screenshot.
- Optional libcosmic rev bump.
