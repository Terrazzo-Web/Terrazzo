use std::string::FromUtf8Error;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;

pub mod ca;
pub mod cert;
pub mod common_fields;
pub mod key;
pub mod name;
pub mod native_roots;
pub mod serial_number;
pub mod signed_extension;
pub mod stack;
pub mod time;
pub mod validity;

pub trait PemString {
    fn pem_string(self) -> Result<String, PemAsStringError>;
}

impl PemString for Vec<u8> {
    fn pem_string(self) -> Result<String, PemAsStringError> {
        Ok(String::from_utf8(self)?)
    }
}

impl PemString for Result<Vec<u8>, ErrorStack> {
    fn pem_string(self) -> Result<String, PemAsStringError> {
        self?.pem_string()
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum PemAsStringError {
    #[error("[{n}] Failed to convert to PEM: {0}", n = self.name())]
    ToPem(#[from] ErrorStack),

    #[error("[{n}] Failed to cast PEM as UTF-8: {0}", n = self.name())]
    FromUtf8(#[from] FromUtf8Error),
}
