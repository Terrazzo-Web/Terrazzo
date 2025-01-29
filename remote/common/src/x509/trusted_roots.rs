use std::sync::OnceLock;

use openssl::x509::store::X509Store;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::store::X509StoreRef;
use openssl::x509::X509;

pub fn trusted_roots() -> &'static X509StoreRef {
    static TRUSTED_ROOTS: OnceLock<X509Store> = OnceLock::new();
    TRUSTED_ROOTS.get_or_init(trusted_roots_impl)
}

fn trusted_roots_impl() -> X509Store {
    let mut trusted_roots = X509StoreBuilder::new().expect("X509StoreBuilder::new()");
    for root_ca in rustls_native_certs::load_native_certs().certs {
        match X509::from_der(&root_ca) {
            Ok(root_ca) => trusted_roots
                .add_cert(root_ca)
                .expect("trusted_roots.add_cert(root_ca)"),
            Err(error) => tracing::trace!("Failed to parse Root CA: {error}"),
        }
    }
    trusted_roots.build()
}

#[cfg(test)]
mod tests {
    #[test]
    fn trusted_roots() {
        let trusted_roots: Vec<_> = super::trusted_roots()
            .all_certificates()
            .into_iter()
            .collect();
        assert!(trusted_roots.len() > 0);
    }
}
