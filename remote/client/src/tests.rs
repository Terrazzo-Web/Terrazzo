#![cfg(test)]

use trz_gateway_common::protos::terrazzo::remote::tests::Expression;
use trz_gateway_common::protos::terrazzo::remote::tests::Value;
use trz_gateway_common::protos::terrazzo::remote::tests::test_tunnel_service_client::TestTunnelServiceClient;

use self::end_to_end::EndToEnd;

mod end_to_end;
mod test_client_config;
mod test_gateway_config;
mod test_tunnel_config;

#[tokio::test]
async fn trivial() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(async |end_to_end| {
        let server = &end_to_end.server;
        let client_id = &end_to_end.client_id;
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
        drop(end_to_end);
        Ok(())
    })
    .await
}

#[tokio::test]
async fn with_sleep() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(async |end_to_end| {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let server = &end_to_end.server;
        let client_id = &end_to_end.client_id;
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
        drop(end_to_end);
        Ok(())
    })
    .await
}
