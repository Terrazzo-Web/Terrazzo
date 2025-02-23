use serde::Deserialize;
use serde::Serialize;

use crate::id::ClientName;

#[derive(Debug, Serialize, Deserialize)]
pub struct GetCertificateRequest<A> {
    pub auth_code: A,
    pub public_key: String,
    pub name: ClientName,
}
