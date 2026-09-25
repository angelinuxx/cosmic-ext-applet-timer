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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(secs)
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
}
