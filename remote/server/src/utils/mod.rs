use std::sync::Arc;

use rustls::crypto::CryptoProvider;
use rustls::crypto::aws_lc_rs;

pub fn crypto_provider() -> &'static Arc<CryptoProvider> {
    let once = std::sync::Once::new();
    once.call_once(|| {
        CryptoProvider::install_default(aws_lc_rs::default_provider())
            .expect("Set default provider")
    });
    CryptoProvider::get_default().expect("Get default provider")
}
