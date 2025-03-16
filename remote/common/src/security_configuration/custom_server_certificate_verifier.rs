use rustls::client::danger::ServerCertVerified;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::ServerName;
use rustls::pki_types::UnixTime;
use tracing::warn;

use super::TrustedStoreConfig;
use crate::x509::signed_extension::validate_signed_extension;

/// A trait to create certificate validators that may or may not have custom validation logic.
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

/// The [CustomServerCertificateVerifier] that has no custom logic, defaults to chain validation.
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

/// The [CustomServerCertificateVerifier] for special client certificates with signed extension.
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
        let () = validate_signed_extension(certificate, &self.store, &self.signer_name)
            .inspect_err(|error| warn!("Failed to validate server certificate: {error}"))?;
        Ok(ServerCertVerified::assertion())
    }
}
