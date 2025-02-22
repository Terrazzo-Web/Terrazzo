#![cfg(test)]

use std::time::Duration;

use trz_gateway_common::protos::terrazzo::remote::tests::Expression;
use trz_gateway_common::protos::terrazzo::remote::tests::Value;
use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_client::TestTunnelServiceClient;

use self::end_to_end::EndToEnd;

mod end_to_end;
mod test_client_config;
mod test_gateway_config;
mod test_tunnel_config;

#[tokio::test]
async fn end_to_end() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(|end_to_end| async move {
        tokio::time::sleep(Duration::from_millis(1)).await;
        let server = end_to_end.server;
        let client_id = end_to_end.client_id;
        let channel = server
            .connections()
            .get_client(&client_id)
            .ok_or(format!("Client {client_id} not found"))?;
        let mut grpc_client = TestTunnelServiceClient::new(channel);
        let response = grpc_client
            .calculate(tonic::Request::new(
                { Expression::from(5) + Expression::from(2) * 3.into() }.into(),
            ))
            .await?
            .into_inner();
        assert_eq!(Value::from(11), response);
        Ok(())
    })
    .await
}
