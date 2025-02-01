use nameth::nameth;
use nameth::NamedEnumValues as _;
use trz_gateway_common::security_configuration::trusted_store::rustls_connector::ToRustlsConnector;
use trz_gateway_common::security_configuration::trusted_store::rustls_connector::ToRustlsConnectorError;
use trz_gateway_common::security_configuration::trusted_store::TrustedStoreConfig;

use super::Client;
use crate::client_configuration::ClientConfig;

impl Client {
    pub async fn connect<C: ClientConfig>(&self, config: C) -> Result<(), ConnectError<C>> {
        let connector = config.server_pki().to_rustls_connector().await?;
        let (web_socket, response) = tokio_tungstenite::connect_async_tls_with_config(
            "",
            None,
            true,
            Some(tokio_tungstenite::Connector::Rustls(connector.into())),
        )
        .await?;
        Ok(())
    }
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum ConnectError<C: ClientConfig> {
    #[error("[{n}] {0}", n = self.name())]
    Certificate(#[from] ToRustlsConnectorError<<C::ServerPkiConfig as TrustedStoreConfig>::Error>),

    #[error("[{n}] {0}", n = self.name())]
    Connect(#[from] tokio_tungstenite::tungstenite::Error),
}
