#![cfg(test)]

use test_gateway_config::TestGatewayConfig;
use trz_gateway_server::server::Server;

use self::test_gateway_config::use_temp_dir;

mod test_gateway_config;

#[tokio::test]
async fn end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    let _use_temp_dir = use_temp_dir();
    let config = TestGatewayConfig::new();
    let handle = Server::run(config.clone()).await?;
    let () = handle.stop("End of test").await?;
    Ok(())
}
