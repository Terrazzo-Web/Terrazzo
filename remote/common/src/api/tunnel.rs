use serde::Deserialize;
use serde::Serialize;

use crate::id::ClientName;

/// Request body of the /remote/certificate API to issue a client certificate.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetCertificateRequest<A, K = String> {
    /// A secret code that the client needs to authenticate.
    pub auth_code: A,

    /// The public key of the certificate (the private key stays with the client).
    pub public_key: K,

    /// Uniquely identifies the client, will be set as the the common name of the issued certificate.
    pub name: ClientName,
}
