use openssl::error::ErrorStack;
use openssl::x509::X509;

pub fn parse_pem_certificates(pems: &str) -> impl Iterator<Item = Result<X509, ErrorStack>> + '_ {
    pems.split_inclusive("-----END CERTIFICATE-----")
        .map(|pem| pem.trim())
        .filter(|pem| !pem.is_empty())
        .map(|pem| X509::from_pem(pem.as_bytes()))
}

pub(super) fn get_or_init<T: Clone, E>(
    mutex: &std::sync::Mutex<Option<T>>,
    make: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let mut lock = mutex.lock().unwrap();
    match &mut *lock {
        Some(value) => Ok(value.clone()),
        None => {
            let value = make()?;
            *lock = Some(value.clone());
            return Ok(value);
        }
    }
}
