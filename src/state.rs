// SPDX-License-Identifier: MPL-2.0

use std::time::Duration;

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

use crate::timer::Timer;

/// Runtime state persisted under `~/.local/state/cosmic/<APP_ID>/`, so running
/// timers survive panel restarts and logout. Version 2 replaced the single
/// timer of version 1 with a list; the old state is not migrated.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 2]
pub struct State {
    pub timers: Vec<Timer>,
    /// Id for the next timer added, so ids are never reused.
    pub next_id: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            timers: vec![Timer::new(0, Duration::from_mins(5))],
            next_id: 1,
        }
    }
}

/// Opens the state store and reads the saved state. A missing or unreadable
/// store falls back to the default state, and there is always at least one
/// timer.
pub fn load(app_id: &str) -> (Option<cosmic_config::Config>, State) {
    let handler = match cosmic_config::Config::new_state(app_id, State::VERSION) {
        Ok(handler) => handler,
        Err(err) => {
            eprintln!("failed to open state store: {err}");
            return (None, State::default());
        }
    };
    let mut state = match State::get_entry(&handler) {
        Ok(state) => state,
        Err((errors, state)) => {
            // A key that was never written is not an error.
            for err in errors.iter().filter(|err| err.is_err()) {
                eprintln!("failed to read state: {err}");
            }
            state
        }
    };
    if state.timers.is_empty() {
        state = State::default();
    }
    (Some(handler), state)
}

/// Writes the state; failures are logged, never fatal.
pub fn save(handler: &cosmic_config::Config, state: &State) {
    if let Err(err) = state.write_entry(handler) {
        eprintln!("failed to save state: {err}");
    }
}
