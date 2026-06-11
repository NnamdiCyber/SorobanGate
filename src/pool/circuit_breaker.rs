use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    state: CircuitBreakerState,
    failure_count: u32,
    failure_threshold: u32,
    cooldown: Duration,
    last_state_change: Instant,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitBreakerState::Closed,
            failure_count: 0,
            failure_threshold,
            cooldown,
            last_state_change: Instant::now(),
        }
    }

    pub fn state(&self) -> CircuitBreakerState {
        self.state
    }

    pub fn is_available(&self) -> bool {
        match self.state {
            CircuitBreakerState::Closed => true,
            CircuitBreakerState::Open => false,
            CircuitBreakerState::HalfOpen => true,
        }
    }

    pub fn try_half_open(&mut self) {
        if self.state == CircuitBreakerState::Open
            && self.last_state_change.elapsed() >= self.cooldown
        {
            self.state = CircuitBreakerState::HalfOpen;
            self.last_state_change = Instant::now();
        }
    }

    pub fn record_success(&mut self) {
        match self.state {
            CircuitBreakerState::HalfOpen => {
                self.state = CircuitBreakerState::Closed;
                self.failure_count = 0;
                self.last_state_change = Instant::now();
            }
            CircuitBreakerState::Closed => {
                self.failure_count = 0;
            }
            CircuitBreakerState::Open => {}
        }
    }

    pub fn record_failure(&mut self) {
        match self.state {
            CircuitBreakerState::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.failure_threshold {
                    self.state = CircuitBreakerState::Open;
                    self.last_state_change = Instant::now();
                }
            }
            CircuitBreakerState::HalfOpen => {
                self.state = CircuitBreakerState::Open;
                self.last_state_change = Instant::now();
            }
            CircuitBreakerState::Open => {
                self.last_state_change = Instant::now();
            }
        }
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    pub fn failure_threshold(&self) -> u32 {
        self.failure_threshold
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_closed_accepts_requests() {
        let cb = CircuitBreaker::new(3, Duration::from_secs(30));
        assert!(cb.is_available());
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_failure_threshold_opens_circuit() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(30));
        assert_eq!(cb.state(), CircuitBreakerState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert_eq!(cb.failure_count(), 1);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert_eq!(cb.failure_count(), 2);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.is_available());
    }

    #[test]
    fn test_open_rejects_requests() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(30));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.is_available());
    }

    #[test]
    fn test_half_open_transition_after_cooldown() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(1));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);

        std::thread::sleep(Duration::from_millis(2));
        cb.try_half_open();
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);
        assert!(cb.is_available());
    }

    #[test]
    fn test_half_open_no_transition_before_cooldown() {
        let mut cb = CircuitBreaker::new(1, Duration::from_secs(30));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);

        cb.try_half_open();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }

    #[test]
    fn test_half_open_success_closes() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(1));
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(2));
        cb.try_half_open();
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        cb.record_success();
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
        assert!(cb.is_available());
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_half_open_failure_reopens() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(1));
        cb.record_failure();
        std::thread::sleep(Duration::from_millis(2));
        cb.try_half_open();
        assert_eq!(cb.state(), CircuitBreakerState::HalfOpen);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
        assert!(!cb.is_available());
    }

    #[test]
    fn test_success_in_closed_resets_failure_count() {
        let mut cb = CircuitBreaker::new(3, Duration::from_secs(30));
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitBreakerState::Closed);
    }

    #[test]
    fn test_record_failure_in_open_resets_timer() {
        let mut cb = CircuitBreaker::new(1, Duration::from_millis(10));
        cb.record_failure();
        assert_eq!(cb.state(), CircuitBreakerState::Open);

        std::thread::sleep(Duration::from_millis(5));
        cb.record_failure(); // resets timer
        std::thread::sleep(Duration::from_millis(5));

        // Should still be Open because timer was reset
        cb.try_half_open();
        assert_eq!(cb.state(), CircuitBreakerState::Open);
    }
}
