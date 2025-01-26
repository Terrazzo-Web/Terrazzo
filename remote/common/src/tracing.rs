use nameth::nameth;
use nameth::NamedEnumValues as _;
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
        .with_max_level(LevelFilter::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;
    debug!("Tracing enabled");

    std::panic::set_hook(Box::new(|panic| warn!("{panic:?}")));
    Ok(())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum EnableTracingError {
    #[error("[{n}] {0}", n = self.name())]
    SetGlobalDefault(#[from] SetGlobalDefaultError),
}
