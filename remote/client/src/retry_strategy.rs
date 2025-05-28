//! Retry strategy.

use std::hash::BuildHasher;
use std::hash::DefaultHasher;
use std::hash::Hasher;
use std::hash::RandomState;
use std::ops::Add;
use std::ops::Mul;
use std::time::Duration;
use std::u64;

/// Retry strategy with exponential backoff.
#[derive(Clone, Debug)]
pub enum RetryStrategy {
    Fixed {
        delay: Duration,
    },
    ExponentialBackoff {
        base: Box<RetryStrategy>,
        exponent: f64,
        max_delay: Duration,
    },
    Random {
        base: Box<RetryStrategy>,
        factor: f64,
        random: DefaultHasher,
    },
    Mult {
        base: Box<RetryStrategy>,
        factor: f64,
    },
    Plus {
        left: Box<RetryStrategy>,
        right: Box<RetryStrategy>,
    },
    Sequence {
        first: Box<RetryStrategy>,
        times: u32,
        then: Box<RetryStrategy>,
    },
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::from(Duration::from_secs(1)).exponential_backoff(2., Duration::from_secs(60))
    }
}

impl From<Duration> for RetryStrategy {
    fn from(delay: Duration) -> Self {
        Self::Fixed { delay }
    }
}

impl RetryStrategy {
    pub fn fixed(delay: Duration) -> Self {
        Self::Fixed { delay }
    }

    pub fn exponential_backoff(self, exponent: f64, max_delay: Duration) -> Self {
        Self::ExponentialBackoff {
            base: Box::new(self),
            exponent,
            max_delay: max_delay.into(),
        }
    }

    pub fn random(self, factor: f64) -> Self {
        Self::Random {
            base: Box::new(self),
            factor,
            random: RandomState::new().build_hasher(),
        }
    }
}

impl Mul<f64> for RetryStrategy {
    type Output = Self;

    fn mul(self, f: f64) -> Self {
        match self {
            Self::Fixed { delay } => Self::Fixed {
                delay: delay.div_f64(f),
            },
            Self::ExponentialBackoff { .. } => Self::Mult {
                base: self.into(),
                factor: f,
            },
            Self::Random {
                base,
                factor,
                random,
            } => RetryStrategy::Random {
                base: Box::new((*base).clone() * f),
                factor,
                random,
            },
            Self::Mult { base, factor } => Self::Mult {
                base,
                factor: factor * f,
            },
            Self::Plus { left, right } => Self::Plus {
                left: Box::new((*left).clone() * f),
                right: Box::new((*right).clone() * f),
            },
            Self::Sequence { first, times, then } => Self::Sequence {
                first: Box::new((*first).clone() * f),
                times,
                then: Box::new((*then).clone() * f),
            },
        }
    }
}

impl Add for RetryStrategy {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Plus {
            left: self.into(),
            right: rhs.into(),
        }
    }
}

impl RetryStrategy {
    pub fn times(self, times: u32) -> RetryStrategyTimes {
        RetryStrategyTimes {
            retry_strategy: self,
            times,
        }
    }
}

impl RetryStrategyTimes {
    pub fn then(self, then: RetryStrategy) -> RetryStrategy {
        RetryStrategy::Sequence {
            first: self.retry_strategy.into(),
            times: self.times,
            then: then.into(),
        }
    }
}

pub struct RetryStrategyTimes {
    retry_strategy: RetryStrategy,
    times: u32,
}

impl RetryStrategy {
    pub fn peek(&self) -> Duration {
        match self {
            RetryStrategy::Fixed { delay } => *delay,
            RetryStrategy::ExponentialBackoff {
                base, max_delay, ..
            } => Ord::min(base.peek(), *max_delay),
            RetryStrategy::Random {
                base,
                factor,
                random,
            } => {
                let r: u64 = random.finish();

                // generate a random number between [0..1]
                let r = r as f64 / (u64::MAX - u64::MIN) as f64;

                // generate a random number between [(1-f/2) .. (1+f/2))]
                let r = 1.0 + *factor * (r - 0.5);

                return base.peek().mul_f64(r);
            }
            RetryStrategy::Mult { base, factor } => base.peek().mul_f64(*factor),
            RetryStrategy::Plus { left, right } => left.peek() + right.peek(),
            RetryStrategy::Sequence { first, times, then } => {
                if *times > 0 { first } else { then }.peek()
            }
        }
    }

    pub fn max_delay(&self) -> Duration {
        match self {
            RetryStrategy::Fixed { delay } => *delay,
            RetryStrategy::ExponentialBackoff { max_delay, .. } => *max_delay,
            RetryStrategy::Random { base, factor, .. } => {
                base.max_delay().mul_f64(1.0 + factor / 2.0)
            }
            RetryStrategy::Mult { base, factor } => base.max_delay().mul_f64(*factor),
            RetryStrategy::Plus { left, right } => left.max_delay() + right.max_delay(),
            RetryStrategy::Sequence { first, then, .. } => {
                Duration::max(first.max_delay(), then.max_delay())
            }
        }
    }

    pub fn delay(&mut self) -> Duration {
        let next = self.peek();
        match self {
            RetryStrategy::Fixed { .. } => (),
            RetryStrategy::ExponentialBackoff {
                base,
                exponent,
                max_delay,
            } => {
                let exponent = *exponent;
                let max_delay = *max_delay;
                let _ = base.delay();
                if next < max_delay {
                    self.multiply(exponent);
                } else {
                    *self = max_delay.into();
                }
            }
            RetryStrategy::Random { base, random, .. } => {
                let _ = base.delay();
                random.write_u128(next.as_nanos());
            }
            RetryStrategy::Mult { base, .. } => {
                let _ = base.delay();
            }
            RetryStrategy::Plus { left, right } => {
                let _ = left.delay();
                let _ = right.delay();
            }
            RetryStrategy::Sequence { first, times, then } => {
                if *times > 0 {
                    if *times == 1 {
                        let d = first.delay();
                        *self = then.as_ref().clone();
                        return d;
                    }
                    first
                } else {
                    then
                }
                .delay();
                *times -= 1;
            }
        }
        next
    }

    pub fn array<const N: usize>(&mut self) -> [Duration; N] {
        let mut result: [Duration; N] = [Duration::ZERO; N];
        for i in 0..N {
            result[i] = self.delay();
        }
        result
    }

    pub fn wait(&mut self) -> tokio::time::Sleep {
        tokio::time::sleep(self.delay())
    }

    fn multiply(&mut self, f: f64) {
        match self {
            RetryStrategy::Fixed { delay } => *delay = delay.mul_f64(f),
            RetryStrategy::ExponentialBackoff { base, .. } | RetryStrategy::Random { base, .. } => {
                base.multiply(f)
            }
            RetryStrategy::Mult { factor, .. } => *factor *= f,
            RetryStrategy::Plus { left, right } => {
                left.multiply(f);
                right.multiply(f);
            }
            RetryStrategy::Sequence { first, then, .. } => {
                first.multiply(f);
                then.multiply(f);
            }
        }
    }
}

impl Iterator for RetryStrategy {
    type Item = Duration;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.delay())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::RetryStrategy;

    #[test]
    fn fixed() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(123));
        assert_eq!(Duration::from_secs(123), retry_strategy.max_delay());
        assert_eq!(Duration::from_secs(123), retry_strategy.peek());
        assert!(matches!(retry_strategy, RetryStrategy::Fixed { .. }));
        assert_eq!(
            [
                Duration::from_secs(123),
                Duration::from_secs(123),
                Duration::from_secs(123)
            ],
            retry_strategy.array::<3>()
        );
    }

    #[test]
    fn exponential_backoff() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(1))
            .exponential_backoff(1.3, Duration::from_secs(3));
        assert!(matches!(
            retry_strategy,
            RetryStrategy::ExponentialBackoff { .. }
        ));
        assert_eq!(
            "1s, 1.3s, 1.69s, 2.197s, 2.8561s, 3s, 3s",
            Vec::from(retry_strategy.array::<7>().map(|d| format!("{d:?}"))).join(", ")
        );
        assert!(matches!(retry_strategy, RetryStrategy::Fixed { .. }));
    }

    #[test]
    fn peek_exponential_backoff() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(1))
            .exponential_backoff(1.3, Duration::from_secs(3));
        assert_eq!("1s", format!("{:?}", retry_strategy.peek()));
        assert_eq!("1s", format!("{:?}", retry_strategy.peek()));
        assert_eq!("1s", format!("{:?}", retry_strategy.delay()));
        assert_eq!("1.3s", format!("{:?}", retry_strategy.peek()));
        assert_eq!("1.3s", format!("{:?}", retry_strategy.peek()));
        assert_eq!("1.3s", format!("{:?}", retry_strategy.delay()));
        assert_eq!(Duration::from_secs(3), retry_strategy.max_delay());
    }

    #[test]
    fn plus_exponential_backoff() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(1))
            .exponential_backoff(1.3, Duration::from_secs(3))
            + RetryStrategy::fixed(Duration::from_secs(1));
        assert_eq!(
            "2s, 2.3s, 2.69s, 3.197s, 3.8561s, 4s, 4s",
            Vec::from(retry_strategy.array::<7>().map(|d| format!("{d:?}"))).join(", ")
        );
        assert_eq!(Duration::from_secs(4), retry_strategy.max_delay());
    }

    #[test]
    fn mult_exponential_backoff() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(1))
            .exponential_backoff(1.3, Duration::from_secs(3))
            * 2.0;
        assert_eq!(
            "2s, 2.6s, 3.38s, 4.394s, 5.7122s, 6s, 6s",
            Vec::from(retry_strategy.array::<7>().map(|d| format!("{d:?}"))).join(", ")
        );
        assert_eq!(Duration::from_secs(6), retry_strategy.max_delay());
    }

    #[test]
    fn random() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(10)).random(0.2);
        assert!(matches!(retry_strategy, RetryStrategy::Random { .. }));
        for _ in 0..500_000 {
            let next = retry_strategy.delay();
            assert!(next <= Duration::from_secs(11));
            assert!(next >= Duration::from_secs(9));
        }

        assert_eq!(retry_strategy.peek(), retry_strategy.peek());
        assert_eq!(retry_strategy.peek(), retry_strategy.delay());
        assert_ne!(retry_strategy.delay(), retry_strategy.peek());
        assert_eq!(Duration::from_secs(11), retry_strategy.max_delay());
    }

    #[test]
    fn sequence() {
        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(1))
            .exponential_backoff(1.3, Duration::from_secs(3))
            .times(5)
            .then(Duration::from_secs(2).into());
        assert!(matches!(retry_strategy, RetryStrategy::Sequence { .. }));
        assert_eq!(
            "1s, 1.3s, 1.69s, 2.197s, 2.8561s, 2s, 2s",
            Vec::from(retry_strategy.array::<7>().map(|d| format!("{d:?}"))).join(", ")
        );
        assert!(matches!(retry_strategy, RetryStrategy::Fixed { .. }));

        let mut retry_strategy = RetryStrategy::from(Duration::from_secs(1))
            .exponential_backoff(1.3, Duration::from_secs(3))
            .times(5)
            .then(Duration::from_secs(2).into())
            * 2.0
            + Duration::from_secs(100).into();
        {
            assert!(matches!(retry_strategy, RetryStrategy::Plus { .. }));
            let RetryStrategy::Plus { left, .. } = &retry_strategy else {
                panic!()
            };
            assert!(matches!(**left, RetryStrategy::Sequence { .. }));
        }

        assert_eq!(
            "102s, 102.6s, 103.38s, 104.394s, 105.7122s, 101s, 101s",
            Vec::from(retry_strategy.array::<7>().map(|d| format!("{d:?}"))).join(", ")
        );

        {
            assert!(matches!(retry_strategy, RetryStrategy::Plus { .. }));
            let RetryStrategy::Plus { left, .. } = &retry_strategy else {
                panic!()
            };
            assert!(matches!(**left, RetryStrategy::Fixed { .. }));
        }
    }
}
