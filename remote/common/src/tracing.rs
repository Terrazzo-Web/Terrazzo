use std::panic::Location;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use tracing::debug;
use tracing::level_filters::LevelFilter;
use tracing::subscriber::SetGlobalDefaultError;
use tracing::warn;

pub fn enable_tracing() -> Result<(), EnableTracingError> {
    let subscriber = tracing_subscriber::fmt()
        .compact()
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .with_max_level(LevelFilter::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    debug!("Tracing enabled");

    std::panic::set_hook(Box::new(|panic_info| {
        let panic_payload: Option<&str> =
            if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
                Some(s)
            } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
                Some(s.as_str())
            } else {
                None
            };
        let location = panic_info
            .location()
            .map(Location::to_string)
            .unwrap_or_else(|| "???".into());
        if let Some(panic_payload) = panic_payload {
            warn!("Panic: {panic_payload} at {location}",);
        } else {
            warn!("Panic at {location}",);
        }
    }));
    Ok(())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum EnableTracingError {
    #[error("[{n}] {0}", n = self.name())]
    SetGlobalDefault(#[from] SetGlobalDefaultError),
}

pub mod test_utils {
    use std::sync::Once;

    pub fn enable_tracing_for_tests() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| super::enable_tracing().unwrap());
    }
}
