//! Rate limiters: token bucket, sliding-window log, and per-platform limiters.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use super::exec::Deadline;
use super::types::{Platform, RateLimitConfig};

/// Why a wait gave up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("rate limit wait cancelled: deadline passed")]
pub struct WaitCancelled;

/// A limiter that admits requests.
pub trait RateLimiter: Send + Sync {
    /// Takes a slot if one is free.
    fn allow(&self) -> bool;

    /// Blocks until a slot is free, or the deadline passes.
    ///
    /// # Errors
    ///
    /// The deadline passes first.
    fn wait(&self, deadline: Deadline) -> Result<(), WaitCancelled>;

    fn limit(&self) -> usize;

    fn remaining(&self) -> usize;

    fn reset(&self) -> Instant;
}

/// Sleeps for `want`, capped by the deadline. Fails once the deadline has passed.
fn sleep_until_free(want: Duration, deadline: Deadline) -> Result<(), WaitCancelled> {
    let left = deadline.remaining().ok_or(WaitCancelled)?;
    thread::sleep(want.min(left));
    Ok(())
}

struct BucketState {
    tokens: f64,
    last_refill: Instant,
}

/// A token bucket that refills `refill_rate` tokens per second up to `capacity`.
pub struct TokenBucket {
    state: Mutex<BucketState>,
    capacity: usize,
    refill_rate: f64,
}

impl TokenBucket {
    #[must_use]
    pub fn new(capacity: usize, refill_rate: f64) -> Self {
        Self {
            state: Mutex::new(BucketState {
                tokens: capacity as f64,
                last_refill: Instant::now(),
            }),
            capacity,
            refill_rate,
        }
    }

    fn refill(&self, state: &mut BucketState) {
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        state.tokens = (state.tokens + elapsed * self.refill_rate).min(self.capacity as f64);
        state.last_refill = now;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BucketState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl RateLimiter for TokenBucket {
    fn allow(&self) -> bool {
        let mut state = self.lock();
        self.refill(&mut state);
        if state.tokens >= 1.0 {
            state.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    fn wait(&self, deadline: Deadline) -> Result<(), WaitCancelled> {
        loop {
            if self.allow() {
                return Ok(());
            }
            let want = {
                let state = self.lock();
                let secs = if self.refill_rate > 0.0 {
                    (1.0 - state.tokens) / self.refill_rate
                } else {
                    1.0
                };
                Duration::from_secs_f64(secs.max(0.0)) + Duration::from_millis(1)
            };
            sleep_until_free(want, deadline)?;
        }
    }

    fn limit(&self) -> usize {
        self.capacity
    }

    fn remaining(&self) -> usize {
        let mut state = self.lock();
        self.refill(&mut state);
        state.tokens as usize
    }

    fn reset(&self) -> Instant {
        let mut state = self.lock();
        self.refill(&mut state);
        if state.tokens >= self.capacity as f64 || self.refill_rate <= 0.0 {
            return Instant::now();
        }
        let secs = (self.capacity as f64 - state.tokens) / self.refill_rate;
        state.last_refill + Duration::from_secs_f64(secs)
    }
}

/// Admits at most `limit` requests in any `window`.
pub struct SlidingWindowLog {
    requests: Mutex<VecDeque<Instant>>,
    limit: usize,
    window: Duration,
}

impl SlidingWindowLog {
    #[must_use]
    pub fn new(limit: usize, window: Duration) -> Self {
        Self {
            requests: Mutex::new(VecDeque::with_capacity(limit)),
            limit,
            window,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, VecDeque<Instant>> {
        self.requests.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn cleanup(&self, requests: &mut VecDeque<Instant>) {
        let now = Instant::now();
        while requests
            .front()
            .is_some_and(|t| now.duration_since(*t) >= self.window)
        {
            requests.pop_front();
        }
    }
}

impl RateLimiter for SlidingWindowLog {
    fn allow(&self) -> bool {
        let mut requests = self.lock();
        self.cleanup(&mut requests);
        if requests.len() < self.limit {
            requests.push_back(Instant::now());
            true
        } else {
            false
        }
    }

    fn wait(&self, deadline: Deadline) -> Result<(), WaitCancelled> {
        loop {
            if self.allow() {
                return Ok(());
            }
            let want = {
                let requests = self.lock();
                requests.front().map_or(Duration::from_millis(1), |t| {
                    (*t + self.window).saturating_duration_since(Instant::now())
                        + Duration::from_millis(1)
                })
            };
            sleep_until_free(want, deadline)?;
        }
    }

    fn limit(&self) -> usize {
        self.limit
    }

    fn remaining(&self) -> usize {
        let mut requests = self.lock();
        self.cleanup(&mut requests);
        self.limit.saturating_sub(requests.len())
    }

    fn reset(&self) -> Instant {
        let mut requests = self.lock();
        self.cleanup(&mut requests);
        requests
            .front()
            .map_or_else(Instant::now, |t| *t + self.window)
    }
}

/// Admits a request only when every limiter does.
pub struct MultiRateLimiter {
    limiters: Vec<Arc<dyn RateLimiter>>,
}

impl MultiRateLimiter {
    #[must_use]
    pub fn new(limiters: Vec<Arc<dyn RateLimiter>>) -> Self {
        Self { limiters }
    }
}

impl RateLimiter for MultiRateLimiter {
    fn allow(&self) -> bool {
        self.limiters.iter().all(|l| l.allow())
    }

    fn wait(&self, deadline: Deadline) -> Result<(), WaitCancelled> {
        self.limiters.iter().try_for_each(|l| l.wait(deadline))
    }

    fn limit(&self) -> usize {
        self.limiters.iter().map(|l| l.limit()).min().unwrap_or(0)
    }

    fn remaining(&self) -> usize {
        self.limiters
            .iter()
            .map(|l| l.remaining())
            .min()
            .unwrap_or(0)
    }

    fn reset(&self) -> Instant {
        self.limiters
            .iter()
            .map(|l| l.reset())
            .max()
            .unwrap_or_else(Instant::now)
    }
}

/// The default limits for each platform.
#[must_use]
pub fn default_rate_limits() -> BTreeMap<Platform, RateLimitConfig> {
    let minute = Duration::from_secs(60);
    let token = |rps: f64, burst: usize| RateLimitConfig {
        requests_per_second: rps,
        burst,
        window: minute,
        algorithm: "token-bucket".to_owned(),
    };
    let mut map = BTreeMap::new();
    map.insert(Platform::Web, token(10.0, 20));
    map.insert(Platform::YouTube, token(5.0, 10));
    map.insert(
        Platform::GitHub,
        RateLimitConfig {
            requests_per_second: 50.0,
            burst: 100,
            window: Duration::from_secs(3600),
            algorithm: "sliding-window".to_owned(),
        },
    );
    map.insert(Platform::Rss, token(2.0, 5));
    map.insert(Platform::Twitter, token(1.0, 3));
    map.insert(Platform::Reddit, token(2.0, 5));
    map.insert(Platform::Bilibili, token(5.0, 10));
    map.insert(Platform::XiaoHongShu, token(1.0, 2));
    map.insert(Platform::LinkedIn, token(2.0, 5));
    map.insert(Platform::ExaSearch, token(5.0, 10));
    map.insert(Platform::V2ex, token(10.0, 20));
    map.insert(Platform::Xueqiu, token(10.0, 20));
    map.insert(Platform::Xiaoyuzhou, token(2.0, 5));
    map.insert(Platform::BossZhipin, token(1.0, 3));
    map.insert(Platform::Facebook, token(1.0, 2));
    map.insert(Platform::Instagram, token(1.0, 2));
    map
}

fn create_limiter(config: &RateLimitConfig) -> Arc<dyn RateLimiter> {
    if config.algorithm == "sliding-window" {
        Arc::new(SlidingWindowLog::new(config.burst, config.window))
    } else {
        Arc::new(TokenBucket::new(config.burst, config.requests_per_second))
    }
}

/// One limiter per platform, built from its configuration.
pub struct PlatformRateLimiter {
    limiters: RwLock<HashMap<Platform, Arc<dyn RateLimiter>>>,
    configs: RwLock<BTreeMap<Platform, RateLimitConfig>>,
}

impl PlatformRateLimiter {
    #[must_use]
    pub fn new(configs: BTreeMap<Platform, RateLimitConfig>) -> Self {
        let limiters = configs
            .iter()
            .map(|(p, c)| (*p, create_limiter(c)))
            .collect();
        Self {
            limiters: RwLock::new(limiters),
            configs: RwLock::new(configs),
        }
    }

    /// The limiter for `platform`. An unconfigured platform gets an effectively unlimited bucket.
    #[must_use]
    pub fn get(&self, platform: Platform) -> Arc<dyn RateLimiter> {
        if let Some(l) = self
            .limiters
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&platform)
        {
            return Arc::clone(l);
        }
        match self
            .configs
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&platform)
        {
            Some(config) => create_limiter(config),
            None => Arc::new(TokenBucket::new(1000, 1000.0)),
        }
    }

    /// Replaces the limit for `platform`.
    pub fn set_config(&self, platform: Platform, config: RateLimitConfig) {
        let limiter = create_limiter(&config);
        self.limiters
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(platform, limiter);
        self.configs
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(platform, config);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bucket_admits_its_burst_then_refuses() {
        let bucket = TokenBucket::new(2, 0.0001);
        assert!(bucket.allow());
        assert!(bucket.allow());
        assert!(!bucket.allow());
    }

    #[test]
    fn a_window_log_admits_its_limit() {
        let log = SlidingWindowLog::new(2, Duration::from_secs(60));
        assert!(log.allow());
        assert!(log.allow());
        assert!(!log.allow());
        assert_eq!(log.remaining(), 0);
    }

    #[test]
    fn a_wait_past_its_deadline_is_cancelled() {
        let bucket = TokenBucket::new(0, 0.0001);
        let deadline = Deadline::after(Duration::ZERO);
        assert_eq!(bucket.wait(deadline), Err(WaitCancelled));
    }
}
