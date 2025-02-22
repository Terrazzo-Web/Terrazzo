use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::ec::EcGroup;
use openssl::ec::EcKey;
use openssl::error::ErrorStack;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::pkey::Private;

pub fn make_key() -> Result<PKey<Private>, MakeKeyError> {
    let group = EcGroup::from_curve_name(Nid::X9_62_PRIME256V1).map_err(MakeKeyError::GetCurve)?;
    let ec_key = EcKey::generate(&group).map_err(MakeKeyError::Generate)?;
    let key = PKey::from_ec_key(ec_key).map_err(MakeKeyError::ToKey)?;
    Ok(key)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum MakeKeyError {
    #[error("[{n}] Failed to get the elliptic curve: {0}", n = self.name())]
    GetCurve(ErrorStack),

    #[error("[{n}] Failed to generate an elliptic curve key: {0}", n = self.name())]
    Generate(ErrorStack),

    #[error("[{n}] Failed to convert the elliptic curve key: {0}", n = self.name())]
    ToKey(ErrorStack),
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use openssl::pkey::PKey;

    use crate::x509::PemString as _;

    #[test]
    fn make_key() -> Result<(), Box<dyn Error>> {
        Ok({
            let private_key = super::make_key()?;
            let public_key = private_key.public_key_to_pem()?;
            let public_key = public_key.pem_string()?;
            let _debug = scopeguard::guard_on_unwind((), |_| {
                println!("Public key is\n{public_key}");
            });
            assert!(public_key.starts_with("-----BEGIN PUBLIC KEY-----"));
            let public_key = PKey::public_key_from_pem(public_key.as_bytes())?;
            assert_eq!(
                (256, 128, 72),
                (
                    public_key.bits(),
                    public_key.security_bits(),
                    public_key.size()
                )
            );
            assert!(public_key.public_eq(&private_key));
        })
    }
}
