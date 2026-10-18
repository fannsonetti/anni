use std::time::{Duration, Instant};

/// A monotonic deadline keeps the timer accurate when rendering is idle.
pub struct FocusTimer {
    pub minutes: u32,
    pub is_break: bool,
    pub task_key: String,
    pub task_title: String,
    pub completed: bool,
    remaining: Duration,
    deadline: Option<Instant>,
}

impl Default for FocusTimer {
    fn default() -> Self {
        Self {
            minutes: 25,
            is_break: false,
            task_key: String::new(),
            task_title: String::new(),
            completed: false,
            remaining: Duration::from_secs(25 * 60),
            deadline: None,
        }
    }
}

impl FocusTimer {
    pub fn running(&self) -> bool {
        self.deadline.is_some()
    }

    pub fn remaining(&self, now: Instant) -> Duration {
        self.deadline
            .map(|end| end.saturating_duration_since(now))
            .unwrap_or(self.remaining)
    }

    pub fn in_progress(&self) -> bool {
        self.running() || (!self.completed && self.remaining < self.duration())
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.minutes as u64 * 60)
    }

    pub fn reset(&mut self) {
        self.deadline = None;
        self.remaining = self.duration();
        self.completed = false;
    }

    pub fn preset(&mut self, minutes: u32, is_break: bool) {
        self.minutes = minutes;
        self.is_break = is_break;
        self.reset();
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.running() {
            self.remaining = self.remaining(now);
            self.deadline = None;
        } else {
            if self.completed {
                self.reset();
            }
            self.deadline = Some(now + self.remaining);
        }
    }

    /// Returns true exactly once at completion (including after a delayed frame).
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.deadline.is_some_and(|end| now >= end) {
            self.deadline = None;
            self.remaining = Duration::ZERO;
            self.completed = true;
            return true;
        }
        false
    }

    pub fn clock(&self, now: Instant) -> String {
        let seconds = self.remaining(now).as_secs_f64().ceil() as u64;
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pause_resume_and_delayed_completion_credit_only_once() {
        let now = Instant::now();
        let mut timer = FocusTimer::default();
        timer.toggle(now);
        timer.toggle(now + Duration::from_secs(60));
        assert_eq!(timer.clock(now + Duration::from_secs(300)), "24:00");
        timer.toggle(now + Duration::from_secs(600));
        assert!(!timer.tick(now + Duration::from_secs(2039)));
        assert!(timer.tick(now + Duration::from_secs(2100)));
        assert!(!timer.tick(now + Duration::from_secs(2200)));
        assert!(timer.completed);
        timer.toggle(now + Duration::from_secs(2200));
        assert_eq!(timer.clock(now + Duration::from_secs(2200)), "25:00");
    }

    #[test]
    fn reset_discards_partial_session_and_preset_clears_completion() {
        let mut timer = FocusTimer::default();
        timer.toggle(Instant::now());
        timer.reset();
        assert!(!timer.in_progress());
        timer.preset(5, true);
        assert_eq!(timer.clock(Instant::now()), "05:00");
        assert!(timer.is_break);
    }
}
