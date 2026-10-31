// SPDX-License-Identifier: MPL-2.0

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

/// User settings stored under `~/.config/cosmic/<APP_ID>/v1/`, one file per
/// field. Edits to those files are picked up live through `watch_config`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CosmicConfigEntry)]
#[version = 1]
pub struct Config {
    /// Quick durations shown in the popup, in minutes.
    pub presets: Vec<u64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            presets: vec![5, 10, 15],
        }
    }
}

/// Reads the settings. Missing keys are written with their defaults so the
/// files exist for the user to edit.
pub fn load(app_id: &str) -> Config {
    let handler = match cosmic_config::Config::new(app_id, Config::VERSION) {
        Ok(handler) => handler,
        Err(err) => {
            eprintln!("failed to open config: {err}");
            return Config::default();
        }
    };
    match Config::get_entry(&handler) {
        Ok(config) => config,
        Err((errors, config)) => {
            let broken: Vec<_> = errors.iter().filter(|err| err.is_err()).collect();
            for err in &broken {
                eprintln!("failed to read config: {err}");
            }
            // Never overwrite a file the user broke; only fill in missing ones.
            if broken.is_empty()
                && let Err(err) = config.write_entry(&handler)
            {
                eprintln!("failed to write default config: {err}");
            }
            config
        }
    }
}
