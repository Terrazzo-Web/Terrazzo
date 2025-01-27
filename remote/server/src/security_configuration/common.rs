use openssl::error::ErrorStack;
use openssl::x509::X509;

pub(super) fn parse_pem_certificates(
    pems: &str,
) -> impl Iterator<Item = Result<X509, ErrorStack>> + '_ {
    pems.split_inclusive("-----END CERTIFICATE-----")
        .map(|pem| pem.trim())
        .filter(|pem| !pem.is_empty())
        .map(|pem| X509::from_pem(pem.as_bytes()))
}
