//! Retry strategy.

use std::time::Duration;

/// Retry strategy with exponential backoff.
#[derive(Clone, Debug)]
pub struct RetryStrategy {
    pub delay: Duration,
    pub exponent: f64,
    pub max_delay: Duration,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            delay: Duration::from_secs(1),
            exponent: 2.,
            max_delay: Duration::from_secs(60),
        }
    }
}

impl RetryStrategy {
    pub async fn wait(&mut self) {
        tokio::time::sleep(self.delay).await;
        self.delay = self.delay.mul_f64(self.exponent).min(self.max_delay);
    }
}
