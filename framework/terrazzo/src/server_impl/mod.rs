pub mod static_assets;

/// Installs assets from the shared library
///
/// i.e. `common.scss` used by the widgets library.
pub fn install_assets() {
    crate::declare_scss_asset!("target/css/common.scss").install();
}

pub use ::axum;
pub use ::http;
pub use ::mime;
