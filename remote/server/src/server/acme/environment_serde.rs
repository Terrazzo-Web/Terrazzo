use instant_acme::LetsEncrypt;
use serde::Deserializer;
use serde::Serializer;

pub fn serialize<S>(environment: &LetsEncrypt, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(match environment {
        LetsEncrypt::Production => "Production",
        LetsEncrypt::Staging => "Staging",
    })
}

pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<LetsEncrypt, D::Error> {
    struct Visitor;

    impl serde::de::Visitor<'_> for Visitor {
        type Value = LetsEncrypt;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a base64-encoded PKCS#8 private key")
        }

        fn visit_str<E>(self, environment: &str) -> Result<LetsEncrypt, E>
        where
            E: serde::de::Error,
        {
            Ok(match environment {
                "Production" => LetsEncrypt::Production,
                "Staging" => LetsEncrypt::Staging,
                _ => {
                    return Err(E::custom(format!(
                        "Unknown LetsEncrypt environment '{environment}'"
                    )));
                }
            })
        }
    }

    deserializer.deserialize_str(Visitor)
}
