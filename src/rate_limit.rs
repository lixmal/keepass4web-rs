use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

// failures before lockouts start
const FREE_FAILURES: u32 = 5;
const BASE_DELAY: Duration = Duration::from_secs(2);
const MAX_DELAY: Duration = Duration::from_secs(15 * 60);

struct Failures {
    count: u32,
    last_failure: Instant,
}

impl Failures {
    fn blocked_until(&self) -> Option<Instant> {
        if self.count <= FREE_FAILURES {
            return None;
        }
        // exponential backoff: 2s, 4s, 8s, ... capped at 15 minutes
        let exponent = (self.count - FREE_FAILURES - 1).min(30);
        let delay = BASE_DELAY.saturating_mul(2u32.saturating_pow(exponent)).min(MAX_DELAY);
        Some(self.last_failure + delay)
    }

    fn expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.last_failure) > MAX_DELAY
    }
}

// in-memory login throttle with exponential backoff,
// keyed by caller-provided strings (e.g. "<ip>|<username>")
#[derive(Default)]
pub struct RateLimiter {
    entries: Mutex<HashMap<String, Failures>>,
}

impl RateLimiter {
    // remaining lockout, if the key is currently blocked
    pub async fn check(&self, key: &str) -> Option<Duration> {
        let entries = self.entries.lock().await;
        let blocked_until = entries.get(key)?.blocked_until()?;
        let now = Instant::now();
        if now < blocked_until {
            return Some(blocked_until - now);
        }
        None
    }

    pub async fn failure(&self, key: &str) {
        let mut entries = self.entries.lock().await;

        // drop stale entries so the map doesn't grow unboundedly
        let now = Instant::now();
        entries.retain(|_, f| !f.expired(now));

        let failures = entries.entry(key.to_string()).or_insert(Failures {
            count: 0,
            last_failure: now,
        });
        failures.count += 1;
        failures.last_failure = now;
    }

    pub async fn success(&self, key: &str) {
        self.entries.lock().await.remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocks_after_free_failures() {
        let limiter = RateLimiter::default();
        let key = "1.2.3.4|alice";

        for _ in 0..FREE_FAILURES {
            limiter.failure(key).await;
            assert!(limiter.check(key).await.is_none());
        }

        limiter.failure(key).await;
        assert!(limiter.check(key).await.is_some());
    }

    #[tokio::test]
    async fn success_resets() {
        let limiter = RateLimiter::default();
        let key = "1.2.3.4|alice";

        for _ in 0..FREE_FAILURES + 3 {
            limiter.failure(key).await;
        }
        assert!(limiter.check(key).await.is_some());

        limiter.success(key).await;
        assert!(limiter.check(key).await.is_none());
    }

    #[tokio::test]
    async fn keys_are_independent() {
        let limiter = RateLimiter::default();

        for _ in 0..FREE_FAILURES + 1 {
            limiter.failure("1.2.3.4|alice").await;
        }
        assert!(limiter.check("1.2.3.4|alice").await.is_some());
        assert!(limiter.check("5.6.7.8|alice").await.is_none());
        assert!(limiter.check("1.2.3.4|bob").await.is_none());
    }

    #[test]
    fn backoff_escalates_and_caps() {
        let now = Instant::now();
        let delay = |count| Failures { count, last_failure: now }
            .blocked_until()
            .map(|until| until - now);

        assert_eq!(delay(FREE_FAILURES), None);
        assert_eq!(delay(FREE_FAILURES + 1), Some(Duration::from_secs(2)));
        assert_eq!(delay(FREE_FAILURES + 2), Some(Duration::from_secs(4)));
        assert_eq!(delay(FREE_FAILURES + 3), Some(Duration::from_secs(8)));
        assert_eq!(delay(FREE_FAILURES + 100), Some(MAX_DELAY));
    }
}
