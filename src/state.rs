// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

use crate::timer::Timer;

/// Runtime state persisted under `~/.local/state/cosmic/<APP_ID>/`, so a
/// running timer survives panel restarts and logout.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct State {
    pub timer: Timer,
}

/// Opens the state store and reads the saved state. A missing or unreadable
/// store falls back to the default state.
pub fn load(app_id: &str) -> (Option<cosmic_config::Config>, State) {
    let handler = match cosmic_config::Config::new_state(app_id, State::VERSION) {
        Ok(handler) => handler,
        Err(err) => {
            eprintln!("failed to open state store: {err}");
            return (None, State::default());
        }
    };
    let state = match State::get_entry(&handler) {
        Ok(state) => state,
        Err((errors, state)) => {
            // A key that was never written is not an error.
            for err in errors.iter().filter(|err| err.is_err()) {
                eprintln!("failed to read state: {err}");
            }
            state
        }
    };
    (Some(handler), state)
}

/// Writes the state; failures are logged, never fatal.
pub fn save(handler: &cosmic_config::Config, state: &State) {
    if let Err(err) = state.write_entry(handler) {
        eprintln!("failed to save state: {err}");
    }
}
