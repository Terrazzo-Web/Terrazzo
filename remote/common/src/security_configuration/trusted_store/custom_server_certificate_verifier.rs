use rustls::client::danger::ServerCertVerified;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::ServerName;
use rustls::pki_types::UnixTime;
use tracing::warn;

use super::TrustedStoreConfig;
use crate::x509::signed_extension::validate_signed_extension;

pub trait CustomServerCertificateVerifier: Send + Sync {
    fn has_custom_logic() -> bool {
        true
    }
    fn verify_server_certificate(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error>;
}

pub struct ChainOnlyServerCertificateVerifier;
impl CustomServerCertificateVerifier for ChainOnlyServerCertificateVerifier {
    fn has_custom_logic() -> bool {
        false
    }
    fn verify_server_certificate(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        unreachable!()
    }
}

pub struct SignedExtensionCertificateVerifier<C: TrustedStoreConfig> {
    pub store: C,
    pub signer_name: String,
}

impl<C: TrustedStoreConfig> CustomServerCertificateVerifier
    for SignedExtensionCertificateVerifier<C>
{
    fn verify_server_certificate(
        &self,
        certificate: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        validate_signed_extension(certificate, &self.store, &self.signer_name)
            .inspect_err(|error| warn!("Failed to validate server certificate: {error}"))
            .map_err(|_| rustls::Error::InvalidCertificate(rustls::CertificateError::BadSignature))
            .map(|()| ServerCertVerified::assertion())
    }
}
