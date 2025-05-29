//! Retry strategy.

use std::hash::DefaultHasher;
use std::hash::Hasher;
use std::ops::Add;
use std::ops::Mul;
use std::time::Duration;

/// Retry strategy with exponential backoff.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]

pub enum RetryStrategy {
    Fixed(#[serde(with = "serde_duration")] Duration),
    ExponentialBackoff(ExponentialBackoff),
    Random(Random),
    Mult(Mult),
    Plus(Plus),
    Sequence(Sequence),
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ExponentialBackoff {
    base: Box<RetryStrategy>,
    exponent: f64,
    #[serde(with = "serde_duration")]
    max_delay: Duration,
}

impl Eq for ExponentialBackoff {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Random {
    base: Box<RetryStrategy>,
    factor: f64,
    #[serde(skip, default = "helpers::new_random")]
    random: DefaultHasher,
}

impl PartialEq for Random {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.factor == other.factor
    }
}

impl Eq for Random {}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Mult {
    base: Box<RetryStrategy>,
    factor: f64,
}

impl PartialEq for Mult {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.factor == other.factor
    }
}

impl Eq for Mult {}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Plus {
    left: Box<RetryStrategy>,
    right: Box<RetryStrategy>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Sequence {
    first: Box<RetryStrategy>,
    times: u32,
    then: Box<RetryStrategy>,
}

mod helpers {
    use std::hash::BuildHasher as _;
    use std::hash::DefaultHasher;

    pub(super) fn new_random() -> DefaultHasher {
        use std::hash::RandomState;
        RandomState::new().build_hasher()
    }
}

mod serde_duration {
    use std::time::Duration;

    use serde::Deserializer;
    use serde::Serializer;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&humantime::format_duration(*duration).to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = Duration;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a Duration")
            }

            fn visit_str<E>(self, duration: &str) -> Result<Duration, E>
            where
                E: serde::de::Error,
            {
                humantime::parse_duration(duration).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self::from(Duration::from_secs(1)).exponential_backoff(2., Duration::from_secs(60))
    }
}

impl From<Duration> for RetryStrategy {
    fn from(delay: Duration) -> Self {
        Self::fixed(delay)
    }
}

impl RetryStrategy {
    pub fn fixed(delay: Duration) -> Self {
        Self::Fixed(delay)
    }

    pub fn exponential_backoff(self, exponent: f64, max_delay: Duration) -> Self {
        Self::ExponentialBackoff(ExponentialBackoff {
            base: Box::new(self),
            exponent,
            max_delay,
        })
    }

    pub fn random(self, factor: f64) -> Self {
        Self::Random(Random {
            base: Box::new(self),
            factor,
            random: helpers::new_random(),
        })
    }
}

impl Mul<f64> for RetryStrategy {
    type Output = Self;

    fn mul(self, f: f64) -> Self {
        match self {
            Self::Fixed(delay) => delay.div_f64(f).into(),
            Self::ExponentialBackoff { .. } => Self::Mult(Mult {
                base: self.into(),
                factor: f,
            }),
            Self::Random(random) => Self::Random(Random {
                base: Box::new((*random.base).clone() * f),
                ..random
            }),
            Self::Mult(mult) => Self::Mult(Mult {
                factor: mult.factor * f,
                ..mult
            }),
            Self::Plus(plus) => Self::Plus(Plus {
                left: Box::new((*plus.left).clone() * f),
                right: Box::new((*plus.right).clone() * f),
            }),
            Self::Sequence(sequence) => Self::Sequence(Sequence {
                first: Box::new((*sequence.first).clone() * f),
                then: Box::new((*sequence.then).clone() * f),
                ..sequence
            }),
        }
    }
}

impl Add for RetryStrategy {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Plus(Plus {
            left: self.into(),
            right: rhs.into(),
        })
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
        RetryStrategy::Sequence(Sequence {
            first: self.retry_strategy.into(),
            times: self.times,
            then: then.into(),
        })
    }
}

pub struct RetryStrategyTimes {
    retry_strategy: RetryStrategy,
    times: u32,
}

impl RetryStrategy {
    pub fn peek(&self) -> Duration {
        match self {
            RetryStrategy::Fixed(delay) => *delay,
            RetryStrategy::ExponentialBackoff(ExponentialBackoff {
                base, max_delay, ..
            }) => Ord::min(base.peek(), *max_delay),
            RetryStrategy::Random(Random {
                base,
                factor,
                random,
            }) => {
                let r: u64 = random.finish();

                // generate a random number between [0..1]
                let r = r as f64 / (u64::MAX - u64::MIN) as f64;

                // generate a random number between [(1-f/2) .. (1+f/2))]
                let r = 1.0 + *factor * (r - 0.5);

                return base.peek().mul_f64(r);
            }
            RetryStrategy::Mult(Mult { base, factor }) => base.peek().mul_f64(*factor),
            RetryStrategy::Plus(Plus { left, right }) => left.peek() + right.peek(),
            RetryStrategy::Sequence(Sequence { first, times, then }) => {
                if *times > 0 { first } else { then }.peek()
            }
        }
    }

    pub fn max_delay(&self) -> Duration {
        match self {
            RetryStrategy::Fixed(delay) => *delay,
            RetryStrategy::ExponentialBackoff(ExponentialBackoff { max_delay, .. }) => *max_delay,
            RetryStrategy::Random(Random { base, factor, .. }) => {
                base.max_delay().mul_f64(1.0 + factor / 2.0)
            }
            RetryStrategy::Mult(Mult { base, factor }) => base.max_delay().mul_f64(*factor),
            RetryStrategy::Plus(Plus { left, right }) => left.max_delay() + right.max_delay(),
            RetryStrategy::Sequence(Sequence { first, then, .. }) => {
                Duration::max(first.max_delay(), then.max_delay())
            }
        }
    }

    pub fn delay(&mut self) -> Duration {
        let next = self.peek();
        match self {
            RetryStrategy::Fixed { .. } => (),
            RetryStrategy::ExponentialBackoff(ExponentialBackoff {
                base,
                exponent,
                max_delay,
            }) => {
                let exponent = *exponent;
                let max_delay = *max_delay;
                let _ = base.delay();
                if next < max_delay {
                    self.multiply(exponent);
                } else {
                    *self = max_delay.into();
                }
            }
            RetryStrategy::Random(Random { base, random, .. }) => {
                let _ = base.delay();
                random.write_u128(next.as_nanos());
            }
            RetryStrategy::Mult(Mult { base, .. }) => {
                let _ = base.delay();
            }
            RetryStrategy::Plus(Plus { left, right }) => {
                let _ = left.delay();
                let _ = right.delay();
            }
            RetryStrategy::Sequence(Sequence { first, times, then }) => {
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
        for next in &mut result {
            *next = self.delay();
        }
        result
    }

    pub fn wait(&mut self) -> tokio::time::Sleep {
        tokio::time::sleep(self.delay())
    }

    fn multiply(&mut self, f: f64) {
        match self {
            RetryStrategy::Fixed(delay) => *delay = delay.mul_f64(f),
            RetryStrategy::ExponentialBackoff(ExponentialBackoff { base, .. })
            | RetryStrategy::Random(Random { base, .. }) => base.multiply(f),
            RetryStrategy::Mult(Mult { factor, .. }) => *factor *= f,
            RetryStrategy::Plus(Plus { left, right }) => {
                left.multiply(f);
                right.multiply(f);
            }
            RetryStrategy::Sequence(Sequence { first, then, .. }) => {
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

    use super::Plus;
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
            let RetryStrategy::Plus(Plus { left, .. }) = &retry_strategy else {
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
            let RetryStrategy::Plus(Plus { left, .. }) = &retry_strategy else {
                panic!()
            };
            assert!(matches!(**left, RetryStrategy::Fixed { .. }));
        }
    }

    #[test]
    fn serde_json() {
        for (retry_strategy, serialized) in [
            (
                RetryStrategy::fixed(Duration::from_secs(1)),
                r#"{
                  "fixed": "1s"
                }"#,
            ),
            (
                RetryStrategy::fixed(Duration::from_secs(1))
                    .exponential_backoff(2., Duration::from_secs(1)),
                r#"{
                  "exponential-backoff": {
                    "base": {
                      "fixed": "1s"
                    },
                    "exponent": 2.0,
                    "max_delay": "1s"
                  }
                }"#,
            ),
            (
                RetryStrategy::fixed(Duration::from_secs(1))
                    .exponential_backoff(2., Duration::from_secs(1))
                    .random(0.3),
                r#"{
                  "random": {
                    "base": {
                      "exponential-backoff": {
                        "base": {
                          "fixed": "1s"
                        },
                        "exponent": 2.0,
                        "max_delay": "1s"
                      }
                    },
                    "factor": 0.3
                  }
                }"#,
            ),
        ] {
            assert_eq!(
                serde_json::to_string_pretty(&retry_strategy)
                    .unwrap()
                    .replace("\n", "\n                "),
                serialized
            );
            assert_eq!(retry_strategy, serde_json::from_str(serialized).unwrap());
        }
    }

    #[test]
    fn serde_toml() {
        for (retry_strategy, serialized) in [
            (
                RetryStrategy::fixed(Duration::from_secs(1)),
                r#"fixed = "1s""#,
            ),
            (
                RetryStrategy::fixed(Duration::from_secs(1))
                    .exponential_backoff(2., Duration::from_secs(1)),
                r#"[exponential-backoff]
                exponent = 2.0
                max_delay = "1s"
                
                [exponential-backoff.base]
                fixed = "1s""#,
            ),
            (
                RetryStrategy::fixed(Duration::from_secs(1))
                    .exponential_backoff(2., Duration::from_secs(1))
                    .random(0.3),
                r#"[random]
                factor = 0.3
                
                [random.base.exponential-backoff]
                exponent = 2.0
                max_delay = "1s"
                
                [random.base.exponential-backoff.base]
                fixed = "1s""#,
            ),
        ] {
            assert_eq!(
                toml::to_string(&retry_strategy)
                    .inspect_err(|e| println!("{e}"))
                    .unwrap()
                    .trim()
                    .replace("\n", "\n                "),
                serialized
            );
            assert_eq!(retry_strategy, toml::from_str(serialized).unwrap());
        }
    }
}
