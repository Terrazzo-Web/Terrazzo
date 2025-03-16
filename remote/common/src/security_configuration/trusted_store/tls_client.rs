use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;
use rustls::ClientConfig;
use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::HandshakeSignatureValid;
use rustls::client::danger::ServerCertVerified;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::ServerName;
use rustls::pki_types::UnixTime;
use rustls::server::VerifierBuilderError;

use super::TrustedStoreConfig;
use super::root_cert_store::ToRootCertStore;
use super::root_cert_store::ToRootCertStoreError;
use crate::security_configuration::custom_server_certificate_verifier::CustomServerCertificateVerifier;

/// TLS client for
/// - Client to Gateway WebSocket
/// - Gateway to Client gRPC + needs custom server cert validator
pub trait ToTlsClient: TrustedStoreConfig + Sized {
    fn to_tls_client(
        &self,
        server_certificate_verifier: impl CustomServerCertificateVerifier + 'static,
    ) -> Result<ClientConfig, ToTlsClientError<Self::Error>> {
        to_tls_client_impl(self, server_certificate_verifier)
    }
}

impl<T: TrustedStoreConfig> ToTlsClient for T {}

fn to_tls_client_impl<T, V>(
    trusted_store_config: &T,
    server_certificate_verifier: V,
) -> Result<ClientConfig, ToTlsClientError<T::Error>>
where
    T: TrustedStoreConfig,
    V: CustomServerCertificateVerifier + 'static,
{
    let root_store = Arc::new(trusted_store_config.to_root_cert_store()?);
    let builder = if V::has_custom_logic() {
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(CustomWebPkiServerVerifier {
                custom: server_certificate_verifier,
                chain: WebPkiServerVerifier::builder(root_store).build()?,
            }))
    } else {
        ClientConfig::builder().with_root_certificates(root_store)
    };
    Ok(builder.with_no_client_auth())
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ToTlsClientError<E: std::error::Error> {
    #[error("[{n}] {0}", n = self.name())]
    ToRootCertStore(#[from] ToRootCertStoreError<E>),

    #[error("[{n}] {0}", n = self.name())]
    VerifierBuilderError(#[from] VerifierBuilderError),
}

#[nameth]
struct CustomWebPkiServerVerifier<T> {
    custom: T,
    chain: Arc<WebPkiServerVerifier>,
}

impl<T: CustomServerCertificateVerifier> rustls::client::danger::ServerCertVerifier
    for CustomWebPkiServerVerifier<T>
{
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        let ServerCertVerified { .. } = self.custom.verify_server_certificate(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;
        self.chain
            .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.chain.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        self.chain.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.chain.supported_verify_schemes()
    }

    fn requires_raw_public_keys(&self) -> bool {
        self.chain.requires_raw_public_keys()
    }

    fn root_hint_subjects(&self) -> Option<&[rustls::DistinguishedName]> {
        self.chain.root_hint_subjects()
    }
}

impl<T> std::fmt::Debug for CustomWebPkiServerVerifier<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(Self::type_name()).finish()
    }
}
