// SPDX-License-Identifier: MIT

use tokio::process::Command;

const EXPIRED_SOUND: &str = "/usr/share/sounds/freedesktop/stereo/alarm-clock-elapsed.oga";

/// Plays the expiry sound through `pw-play` and waits for it to finish.
/// A missing player or sound file is logged, never fatal.
pub async fn play_expired() {
    match Command::new("pw-play").arg(EXPIRED_SOUND).status().await {
        Ok(status) if !status.success() => eprintln!("pw-play exited with {status}"),
        Ok(_) => {}
        Err(err) => eprintln!("failed to run pw-play: {err}"),
    }
}
