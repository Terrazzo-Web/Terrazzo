use axum::http::StatusCode;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::error::ErrorStack;
use openssl::nid::Nid;
use openssl::x509::X509Name;
use openssl::x509::X509NameBuilder;

use crate::http_error::IsHttpError;

pub(super) fn make_name(args: CertitficateName) -> Result<X509Name, MakeNameError> {
    let mut name = X509NameBuilder::new().map_err(MakeNameError::NewBuilder)?;
    let mut set = |nid, value| {
        let Some(value) = value else { return Ok(()) };
        match name.append_entry_by_nid(nid, value) {
            Ok(()) => Ok(()),
            Err(error) => {
                let nid = nid
                    .long_name()
                    .map_err(|error| MakeNameError::InvalidField { error, nid })?
                    .to_owned();
                let value = value.to_owned();
                Err(MakeNameError::InvalidValue { error, nid, value })
            }
        }
    };
    let country = args.country.map(String::from_iter);
    set(Nid::COUNTRYNAME, country.as_deref())?;
    set(Nid::STATEORPROVINCENAME, args.state_or_province)?;
    set(Nid::LOCALITYNAME, args.locality)?;
    set(Nid::ORGANIZATIONNAME, args.organization)?;
    set(Nid::COMMONNAME, args.common_name)?;
    Ok(name.build())
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct CertitficateName<'t> {
    pub country: Option<[char; 2]>,
    pub state_or_province: Option<&'t str>,
    pub locality: Option<&'t str>,
    pub organization: Option<&'t str>,
    pub common_name: Option<&'t str>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeNameError {
    #[error("[{n}] Failed to create a new LDAP Name builder: {0}", n = self.name())]
    NewBuilder(ErrorStack),

    #[error("[{n}] Failed to set LDAP field {nid} = '{value}': {error}", n = self.name())]
    InvalidValue {
        error: ErrorStack,
        nid: String,
        value: String,
    },

    #[error("[{n}] Invalid LDAP field NID={nid}: {error}", n = self.name(), nid = nid.as_raw())]
    InvalidField { error: ErrorStack, nid: Nid },
}

impl IsHttpError for MakeNameError {
    fn status_code(&self) -> StatusCode {
        match self {
            MakeNameError::NewBuilder { .. } => StatusCode::INTERNAL_SERVER_ERROR,
            MakeNameError::InvalidValue { .. } => StatusCode::BAD_REQUEST,
            MakeNameError::InvalidField { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
#[cfg(test)]
mod tests {

    #[test]
    fn common_name() {
        let name = super::make_name(super::CertitficateName {
            common_name: Some("Test cert"),
            ..Default::default()
        })
        .unwrap();
        let name: String = name.entries().map(|entry| format!("{entry:?}")).collect();
        assert_eq!("\"commonName = \\\"Test cert\\\"\"", format!("{name:?}"));
    }

    #[test]
    fn country() {
        let name = super::make_name(super::CertitficateName {
            common_name: Some("Test cert"),
            country: Some(['F', 'R']),
            ..Default::default()
        })
        .unwrap();
        let name: String = name.entries().map(|entry| format!("{entry:?}")).collect();
        assert_eq!(
            "\"countryName = \\\"FR\\\"commonName = \\\"Test cert\\\"\"",
            format!("{name:?}")
        );
    }

    #[test]
    fn error() {
        let too_long: String = (0..200).map(|_| 'X').collect();
        let Err(error) = super::make_name(super::CertitficateName {
            common_name: Some(&too_long),
            ..Default::default()
        }) else {
            panic!();
        };
        let super::MakeNameError::InvalidValue { nid, value, .. } = &error else {
            panic!();
        };
        assert_eq!(&too_long, value);
        assert_eq!("commonName", nid);
        assert!(error.to_string().starts_with(&format!(
            "[InvalidValue] Failed to set LDAP field commonName = '{too_long}': "
        ),));
    }
}
