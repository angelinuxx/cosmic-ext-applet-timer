// SPDX-License-Identifier: MPL-2.0

use std::time::{Duration, SystemTime};

#[derive(Debug, PartialEq)]
pub enum Event {
    Expired,
}

pub enum TimerState {
    Idle,
    Running { ends_at: SystemTime },
    Paused { remaining: Duration },
    Expired,
}

pub struct Timer {
    pub duration: Duration,
    pub state: TimerState,
}

impl Timer {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            state: TimerState::Idle,
        }
    }

    pub fn start(&mut self, now: SystemTime) {
        self.state = TimerState::Running {
            ends_at: now + self.duration,
        };
    }

    pub fn pause(&mut self, now: SystemTime) {
        self.state = TimerState::Paused {
            remaining: self.remaining(now),
        };
    }

    pub fn resume(&mut self, now: SystemTime) {
        self.state = TimerState::Running {
            ends_at: now + self.remaining(now),
        };
    }

    pub fn tick(&mut self, now: SystemTime) -> Option<Event> {
        match self.state {
            TimerState::Running { ends_at } if now >= ends_at => {
                self.state = TimerState::Expired;
                Some(Event::Expired)
            }
            _ => None,
        }
    }

    pub fn reset(&mut self) {
        self.state = TimerState::Idle;
    }

    pub fn remaining(&self, now: SystemTime) -> Duration {
        match self.state {
            TimerState::Idle => self.duration,
            TimerState::Running { ends_at } => {
                ends_at.duration_since(now).unwrap_or(Duration::ZERO)
            }
            TimerState::Paused { remaining } => remaining,
            TimerState::Expired => Duration::ZERO,
        }
    }

    pub fn progress(&self, now: SystemTime) -> f32 {
        let elapsed = self.duration.saturating_sub(self.remaining(now));
        let duration = self.duration.as_secs_f32();
        if duration == 0.0 {
            return 1.0;
        }
        elapsed.as_secs_f32() / duration
    }

    pub fn set_duration(&mut self, duration: Duration) {
        if matches!(self.state, TimerState::Idle) {
            self.duration = duration;
        }
    }
}

pub fn format_remaining(remaining: Duration) -> String {
    let total = remaining.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < 1e-6,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn new_timer_is_idle_with_full_duration() {
        let timer = Timer::new(Duration::from_secs(300));
        assert!(matches!(timer.state, TimerState::Idle));
        assert_eq!(timer.remaining(at(0)), Duration::from_secs(300));
    }

    #[test]
    fn started_timer_counts_down() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        assert!(matches!(timer.state, TimerState::Running { .. }));
        assert_eq!(timer.remaining(at(110)), Duration::from_secs(290));
    }

    #[test]
    fn paused_timer_freezes_remaining() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        timer.pause(at(130));

        assert!(matches!(timer.state, TimerState::Paused { .. }));
        assert_eq!(timer.remaining(at(200)), Duration::from_secs(270));
    }

    #[test]
    fn resumed_timer_continues_from_frozen_remaining() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        timer.pause(at(130));
        timer.resume(at(200));

        assert!(matches!(timer.state, TimerState::Running { .. }));
        assert_eq!(timer.remaining(at(210)), Duration::from_secs(260));
    }

    #[test]
    fn tick_before_deadline_does_nothing() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));

        assert_eq!(timer.tick(at(250)), None);
        assert!(matches!(timer.state, TimerState::Running { .. }));
    }

    #[test]
    fn tick_at_deadline_expires_and_reports_it() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));

        assert_eq!(timer.tick(at(400)), Some(Event::Expired));
        assert!(matches!(timer.state, TimerState::Expired));
        assert_eq!(timer.remaining(at(400)), Duration::ZERO);
    }

    #[test]
    fn reset_running_timer_returns_to_idle_with_full_duration() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        timer.reset();

        assert!(matches!(timer.state, TimerState::Idle));
        assert_eq!(timer.remaining(at(250)), Duration::from_secs(300));
    }

    #[test]
    fn reset_expired_timer_returns_to_idle() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        timer.tick(at(400));
        timer.reset();

        assert!(matches!(timer.state, TimerState::Idle));
    }

    #[test]
    fn progress_is_zero_when_idle() {
        let timer = Timer::new(Duration::from_secs(300));
        assert_close(timer.progress(at(0)), 0.0);
    }

    #[test]
    fn progress_is_half_way_through_a_running_timer() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        assert_close(timer.progress(at(250)), 0.5);
    }

    #[test]
    fn progress_is_one_when_expired() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        timer.tick(at(400));
        assert_close(timer.progress(at(400)), 1.0);
    }

    #[test]
    fn set_duration_changes_an_idle_timer() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.set_duration(Duration::from_secs(600));
        assert_eq!(timer.remaining(at(0)), Duration::from_secs(600));
    }

    #[test]
    fn set_duration_is_ignored_while_running() {
        let mut timer = Timer::new(Duration::from_secs(300));
        timer.start(at(100));
        timer.set_duration(Duration::from_secs(600));
        assert_eq!(timer.remaining(at(110)), Duration::from_secs(290));
    }

    #[test]
    fn formats_minutes_and_seconds_with_leading_zeros() {
        assert_eq!(format_remaining(Duration::from_secs(290)), "04:50");
    }

    #[test]
    fn formats_zero() {
        assert_eq!(format_remaining(Duration::ZERO), "00:00");
    }

    #[test]
    fn formats_hours_when_over_sixty_minutes() {
        assert_eq!(format_remaining(Duration::from_secs(3_725)), "1:02:05");
    }
}
