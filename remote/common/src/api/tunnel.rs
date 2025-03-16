use serde::Deserialize;
use serde::Serialize;

use crate::id::ClientName;

/// Request body of the /remote/certificate API to issue a client certificate.
///
/// - `auth_code` is a secret code that the client needs to authenticate.
/// - `public_key` is the public key of the certificate (the private key stays with the client)
/// - `name` uniquely identifies the client, will be set as the the common name of the issued certificate.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetCertificateRequest<A, K = String> {
    pub auth_code: A,
    pub public_key: K,
    pub name: ClientName,
}
