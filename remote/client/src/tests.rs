#![cfg(test)]

use futures::future::join_all;
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
    EndToEnd::run(async |end_to_end| run_trivial_test(&end_to_end).await).await
}

#[tokio::test]
async fn with_sleep() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(async |end_to_end| {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        run_trivial_test(&end_to_end).await
    })
    .await
}

#[tokio::test]
async fn with_close_client() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(async |end_to_end| {
        let () = run_trivial_test(&end_to_end).await?;
        let () = (end_to_end.client_handle)()
            .stop("Stopping the client")
            .await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn with_two_clients() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(async |end_to_end| {
        let handle = end_to_end.client.run().await?;
        let () = run_trivial_test(&end_to_end).await?;
        handle.stop("Stopping the second client").await?;
        Ok(())
    })
    .await
}

#[tokio::test]
async fn with_many_calls() -> Result<(), Box<dyn std::error::Error>> {
    EndToEnd::run(async |end_to_end| {
        let _handle = end_to_end.client.run().await?;
        for result in join_all((0..100).map(|_| run_trivial_test(&end_to_end))).await {
            let () = result?;
        }
        let () = (end_to_end.client_handle)()
            .stop("Stopping the client")
            .await?;
        Ok(())
    })
    .await
}

async fn run_trivial_test(end_to_end: &EndToEnd<'_>) -> Result<(), Box<dyn std::error::Error>> {
    let server = &end_to_end.server;
    let client_name = &end_to_end.client.client_name;
    let channel = server
        .connections()
        .get_client(&client_name)
        .ok_or(format!("Client {client_name} not found"))?;
    let mut grpc_client = TestTunnelServiceClient::new(channel);
    let response = grpc_client
        .calculate(tonic::Request::new(
            { Expression::from(5) + Expression::from(2) * 3.into() }.into(),
        ))
        .await?
        .into_inner();
    assert_eq!(Value::from(11), response);
    Ok(())
}
