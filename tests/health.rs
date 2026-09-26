use std::{net::TcpListener, time::Duration};

use tonic::transport::Endpoint;
use tonic_health::pb::{
    HealthCheckRequest, health_check_response::ServingStatus, health_client::HealthClient,
};

#[tokio::test]
async fn health_check_reports_serving() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_JOT-Container"));
    command
        .env("JOT_GRPC_ADDR", address.to_string())
        .kill_on_drop(true);
    let mut server = command.spawn().unwrap();

    let endpoint = Endpoint::from_shared(format!("http://{address}")).unwrap();
    let channel = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match endpoint.connect().await {
                Ok(channel) => break channel,
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("gRPC server did not start");
    let mut client = HealthClient::new(channel);

    let response = client
        .check(HealthCheckRequest {
            service: String::new(),
        })
        .await
        .unwrap();
    assert_eq!(response.into_inner().status, ServingStatus::Serving as i32);

    server.kill().await.unwrap();
}
