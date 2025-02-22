use nameth::NamedEnumValues as _;
use nameth::nameth;
use openssl::asn1::Asn1Integer;
use openssl::bn::BigNum;
use openssl::error::ErrorStack;
use openssl::x509::X509Builder;

pub(super) fn set_serial_number(builder: &mut X509Builder) -> Result<(), SetSerialNumberError> {
    (|| {
        let mut bytes = vec![0; 20];
        openssl::rand::rand_bytes(&mut bytes).map_err(SetSerialNumberError::RandBytes)?;
        let serial_number =
            BigNum::from_slice(&bytes).map_err(SetSerialNumberError::BigNumFromSlice)?;
        let serial_number =
            Asn1Integer::from_bn(&serial_number).map_err(SetSerialNumberError::ToAsn1)?;
        builder
            .set_serial_number(&serial_number)
            .map_err(SetSerialNumberError::Set)?;
        Ok(())
    })()
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum SetSerialNumberError {
    #[error("[{n}] Failed generate a ramdom serial number: {0}", n = self.name())]
    RandBytes(ErrorStack),

    #[error("[{n}] Failed to convert to BigNum: {0}", n = self.name())]
    BigNumFromSlice(ErrorStack),

    #[error("[{n}] Failed to convert to Asn1: {0}", n = self.name())]
    ToAsn1(ErrorStack),

    #[error("[{n}] Failed to set the serial number: {0}", n = self.name())]
    Set(ErrorStack),
}
