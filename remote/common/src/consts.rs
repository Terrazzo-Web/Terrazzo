use std::time::Duration;

pub const HEALTH_CHECK_TIMEOUT: Duration = if cfg!(debug_assertions) {
    Duration::from_secs(2)
} else {
    Duration::from_secs(5)
};

pub const HEALTH_CHECK_PERIOD: Duration = if cfg!(debug_assertions) {
    Duration::from_secs(10)
} else {
    Duration::from_secs(3 * 60 + 45)
};
