use std::sync::Arc;
use std::sync::OnceLock;

use openssl::x509::X509;
use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;

pub fn native_roots() -> &'static Arc<X509Store> {
    static NATIVE_ROOTS: OnceLock<Arc<X509Store>> = OnceLock::new();
    NATIVE_ROOTS.get_or_init(native_roots_impl)
}

fn native_roots_impl() -> Arc<X509Store> {
    let mut native_roots = X509StoreBuilder::new().expect("X509StoreBuilder::new()");
    for root_ca in rustls_native_certs::load_native_certs().certs {
        match X509::from_der(&root_ca) {
            Ok(root_ca) => native_roots
                .add_cert(root_ca)
                .expect("trusted_roots.add_cert(root_ca)"),
            Err(error) => tracing::trace!("Failed to parse Root CA: {error}"),
        }
    }
    Arc::new(native_roots.build())
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_roots() {
        let native_roots: Vec<_> = super::native_roots()
            .all_certificates()
            .into_iter()
            .collect();
        assert!(native_roots.len() > 0);
    }
}
