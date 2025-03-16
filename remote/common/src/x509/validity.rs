use std::ops::Deref;
use std::time::SystemTime;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::asn1::Asn1Time;
use openssl::asn1::Asn1TimeRef;
use openssl::error::ErrorStack;
use openssl::x509::X509Builder;
use openssl::x509::X509Ref;

use super::time::Asn1ToSystemTimeError;
use super::time::SystemToAsn1TimeError;
use super::time::asn1_to_system_time;
use super::time::system_to_asn1_time;

/// Represents the interval of time for certificate validity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Validity<T = SystemTime> {
    pub from: T,
    pub to: T,
}

pub(super) fn set_validity(
    builder: &mut X509Builder,
    validity: Validity<Asn1Time>,
) -> Result<(), ValidityError<ErrorStack>> {
    builder
        .set_not_before(&validity.from)
        .map_err(ValidityError::NotBefore)?;
    builder
        .set_not_after(&validity.to)
        .map_err(ValidityError::NotAfter)?;
    Ok(())
}

impl<T> Validity<T> {
    pub fn try_map<F, U, E>(self, f: F) -> Result<Validity<U>, ValidityError<E>>
    where
        F: Fn(T) -> Result<U, E>,
        E: std::error::Error,
    {
        Ok(Validity {
            from: f(self.from).map_err(ValidityError::NotBefore)?,
            to: f(self.to).map_err(ValidityError::NotAfter)?,
        })
    }

    pub fn map<F, U>(self, f: F) -> Validity<U>
    where
        F: Fn(T) -> U,
    {
        Validity {
            from: f(self.from),
            to: f(self.to),
        }
    }
}

impl TryFrom<Validity> for Validity<Asn1Time> {
    type Error = ValidityError<SystemToAsn1TimeError>;

    fn try_from(value: Validity) -> Result<Self, Self::Error> {
        value.try_map(system_to_asn1_time)
    }
}

impl<'t> TryFrom<Validity<&'t Asn1TimeRef>> for Validity {
    type Error = ValidityError<Asn1ToSystemTimeError>;

    fn try_from(value: Validity<&'t Asn1TimeRef>) -> Result<Self, Self::Error> {
        value.try_map(asn1_to_system_time)
    }
}

impl<'t> TryFrom<&'t X509Ref> for Validity {
    type Error = ValidityError<Asn1ToSystemTimeError>;

    fn try_from(x509: &'t X509Ref) -> Result<Self, Self::Error> {
        Validity {
            from: x509.not_before(),
            to: x509.not_after(),
        }
        .try_map(asn1_to_system_time)
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ValidityError<E: std::error::Error> {
    #[error("[{n}] Failed to process the 'not before' field: {0}", n = self.name())]
    NotBefore(E),

    #[error("[{n}] Failed to process the 'not after' field: {0}", n = self.name())]
    NotAfter(E),
}

impl<T> Validity<T> {
    pub fn as_deref(&self) -> Validity<&T::Target>
    where
        T: Deref,
    {
        Validity {
            from: &self.from,
            to: &self.to,
        }
    }
}

#[cfg(test)]
mod tests {
    use openssl::asn1::Asn1Time;

    use super::Validity;

    #[test]
    fn convert() -> Result<(), Box<dyn std::error::Error>> {
        for _ in 0..100 {
            let one_year = Validity {
                from: Asn1Time::days_from_now(0)?,
                to: Asn1Time::days_from_now(365)?,
            };
            let system_time: Validity = Validity {
                from: Asn1Time::days_from_now(0)?,
                to: Asn1Time::days_from_now(365)?,
            }
            .as_deref()
            .try_into()?;
            let one_year2: Validity<Asn1Time> = system_time.try_into()?;
            if one_year == one_year2 {
                return Ok(());
            }
        }
        panic!()
    }
}
