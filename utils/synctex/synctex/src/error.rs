use std::ffi::NulError;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo_synctex_sys as sys;

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("[{n}] Path is not valid UTF-8", n = self.name())]
    InvalidPath,

    #[error("[{n}] Path contains an interior NUL byte: {0}", n = self.name())]
    Nul(#[from] NulError),

    #[error("[{n}] Failed to open SyncTeX scanner", n = self.name())]
    OpenFailed,

    #[error("[{n}] Failed to parse SyncTeX file", n = self.name())]
    ParseFailed,

    #[error("[{n}] SyncTeX query failed with status {0}", n = self.name())]
    Status(sys::synctex_status_t),
}
