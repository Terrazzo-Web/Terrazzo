use std::io;
use std::io::Write as _;
use std::path::Path;

use openssl::pkcs12::Pkcs12;

/// Prints the PFX's certificate, including extension details, to stdout.
/// Pass an empty password for an unprotected PFX. Private keys are never printed.
#[expect(dead_code, reason = "Standalone helper for inspecting PFX files")]
pub fn print_pfx(path: impl AsRef<Path>, password: &str) -> io::Result<()> {
    let data = std::fs::read(path)?;
    let pfx = Pkcs12::from_der(&data)?.parse2(password)?;
    let certificate = pfx
        .cert
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PFX contains no certificate"))?;
    let text = crate::converter::certificate_text(&certificate)?;
    io::stdout().lock().write_all(text.as_bytes())
}
