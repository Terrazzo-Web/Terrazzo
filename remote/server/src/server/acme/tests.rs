#![cfg(test)]

use instant_acme::LetsEncrypt;
use trz_gateway_common::crypto_provider::crypto_provider;
use trz_gateway_common::tracing::test_utils::enable_tracing_for_tests;

use crate::server::acme::active_challenges::ActiveChallenges;

#[allow(unused)]
// #[tokio::test]
async fn get_certificate() {
    enable_tracing_for_tests();
    crypto_provider();
    let result = super::AcmeConfig {
        environment: LetsEncrypt::Staging,
        credentials: None.into(),
        contact: "mailto:info@pavy.one".into(),
        domain: "pavy.one".into(),
        certificate: None,
    }
    .get_certificate(&ActiveChallenges::default())
    .await
    .unwrap();
    println!(
        "{}",
        serde_json::to_string_pretty(&result.credentials).unwrap()
    );
}

#[test]
fn environment_serialize_deserialize() {
    use super::environment_serde;

    #[derive(serde::Serialize, serde::Deserialize)]
    struct LetsEncryptConfig {
        #[serde(with = "environment_serde")]
        environment: LetsEncrypt,
    }

    const PROD_VALUE: &str = "{\"environment\":\"Production\"}";
    const STAGE_VALUE: &str = "{\"environment\":\"Staging\"}";
    assert_eq!(
        PROD_VALUE,
        serde_json::to_string(&LetsEncryptConfig {
            environment: LetsEncrypt::Production
        })
        .unwrap()
    );
    assert_eq!(
        STAGE_VALUE,
        serde_json::to_string(&LetsEncryptConfig {
            environment: LetsEncrypt::Staging
        })
        .unwrap()
    );

    assert!(matches!(
        serde_json::from_str::<LetsEncryptConfig>(PROD_VALUE)
            .unwrap()
            .environment,
        LetsEncrypt::Production,
    ));
    assert!(matches!(
        serde_json::from_str::<LetsEncryptConfig>(STAGE_VALUE)
            .unwrap()
            .environment,
        LetsEncrypt::Staging,
    ));
}
