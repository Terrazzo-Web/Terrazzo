use std::num::TryFromIntError;
use std::sync::OnceLock;
use std::time::Duration;
use std::time::SystemTime;
use std::time::SystemTimeError;
use std::time::UNIX_EPOCH;

use nameth::nameth;
use nameth::NamedEnumValues as _;
use openssl::asn1::Asn1Time;
use openssl::asn1::Asn1TimeRef;
use openssl::error::ErrorStack;

pub fn system_to_asn1_time(system: SystemTime) -> Result<Asn1Time, SystemToAsn1TimeError> {
    let unix_seconds = system.duration_since(UNIX_EPOCH)?.as_secs();
    Ok(Asn1Time::from_unix(unix_seconds as i64)?)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SystemToAsn1TimeError {
    #[error("[{n}] Can't convert time earlier than UNIX_EPOCH: {0}", n = self.name())]
    SystemTimeError(#[from] SystemTimeError),

    #[error("[{n}] Failed to convert UNIX seconds to Asn1Time: {0}", n = self.name())]
    Asn1TimeError(#[from] ErrorStack),
}

pub fn asn1_to_system_time(asn1: &Asn1TimeRef) -> Result<SystemTime, Asn1ToSystemTimeError> {
    static UNIX_EPOCH_ASN1TIME: OnceLock<Asn1Time> = OnceLock::new();
    let epoch = UNIX_EPOCH_ASN1TIME
        .get_or_init(|| system_to_asn1_time(UNIX_EPOCH).expect("UNIX_EPOCH as Asn1Time"));
    let diff = epoch.diff(asn1)?;

    let days = diff.days.try_into()?;
    let secs = diff.secs.try_into()?;

    const SECOND: Duration = Duration::from_secs(1);
    Ok(SystemTime::UNIX_EPOCH + SECOND * 3600 * 24 * days + SECOND * secs)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum Asn1ToSystemTimeError {
    #[error("[{n}] Can't convert time earlier than UNIX_EPOCH: {0}", n = self.name())]
    SystemTimeError(#[from] TryFromIntError),

    #[error("[{n}] Failed to convert Asn1Time to UNIX seconds: {0}", n = self.name())]
    Asn1TimeError(#[from] ErrorStack),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    use super::{asn1_to_system_time, system_to_asn1_time};

    #[test]
    fn convert() {
        let system = SystemTime::now();
        let system = system
            - Duration::from_nanos(system.duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as u64);
        let asn1 = system_to_asn1_time(system).unwrap();
        let system2 = asn1_to_system_time(&asn1).unwrap();
        assert_eq!(system, system2);
    }

    #[test]
    fn fifty_years() {
        let fifty = SystemTime::now() + Duration::from_secs(1) * 3600 * 24 * 365 * 50;
        let fifty = fifty
            - Duration::from_nanos(fifty.duration_since(UNIX_EPOCH).unwrap().subsec_nanos() as u64);
        let asn1 = system_to_asn1_time(fifty).unwrap();
        let system = asn1_to_system_time(&asn1).unwrap();
        assert_eq!(fifty, system);
    }
}
