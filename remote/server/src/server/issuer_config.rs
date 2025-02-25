use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::nid::Nid;
use openssl::stack::Stack;
use openssl::x509::X509;
use trz_gateway_common::certificate_info::X509CertificateInfo;
use trz_gateway_common::security_configuration::certificate::CertificateConfig;
use trz_gateway_common::x509::stack::MakeStackError;
use trz_gateway_common::x509::stack::make_stack;
use trz_gateway_common::x509::time::Asn1ToSystemTimeError;
use trz_gateway_common::x509::validity::Validity;
use trz_gateway_common::x509::validity::ValidityError;

pub(super) struct IssuerConfig {
    pub signer: Arc<X509CertificateInfo>,
    pub signer_name: String,
    pub intermediates: Stack<X509>,
    pub validity: Validity,
}

impl IssuerConfig {
    pub(crate) fn new<C: CertificateConfig>(
        client_certificate_issuer: &C,
    ) -> Result<Self, IssuerConfigError<C>> {
        let signer = client_certificate_issuer
            .certificate()
            .map_err(IssuerConfigError::Signer)?;
        let signer_name = signer
            .certificate
            .subject_name()
            .entries_by_nid(Nid::COMMONNAME)
            .next()
            .ok_or(IssuerConfigError::SignerNameNotFound)?
            .data()
            .as_utf8()
            .map_err(IssuerConfigError::SignerNameInvalid)?
            .to_string();
        let intermediates = client_certificate_issuer
            .intermediates()
            .map_err(IssuerConfigError::Intermediates)?;
        let intermediates = make_stack(intermediates.as_ref().iter().cloned())
            .map_err(IssuerConfigError::MakeIntermediatesStack)?;
        let validity = signer.certificate.as_ref().try_into()?;
        Ok(Self {
            signer,
            intermediates,
            validity,
            signer_name,
        })
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum IssuerConfigError<C: CertificateConfig> {
    #[error("[{n}] {0}", n = self.name())]
    Signer(C::Error),

    #[error("[{n}] Signer name not found", n = self.name())]
    SignerNameNotFound,

    #[error("[{n}] {0}", n = self.name())]
    SignerNameInvalid(ErrorStack),

    #[error("[{n}] {0}", n = self.name())]
    Intermediates(C::Error),

    #[error("[{n}] {0}", n = self.name())]
    MakeIntermediatesStack(MakeStackError),

    #[error("[{n}] {0}", n = self.name())]
    Validity(#[from] ValidityError<Asn1ToSystemTimeError>),
}
