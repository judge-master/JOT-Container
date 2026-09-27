use std::net::SocketAddr;

use tonic::transport::Server;
use tonic_health::{ServingStatus, server::health_reporter};

pub async fn serve(address: SocketAddr) -> Result<(), tonic::transport::Error> {
    // 일단 health_reporter를 사용하여 gRPC 서버의 healthchek 응답을 결정하고, 나중에 컴파일러나 Wasm 런타임 환경에 대한 정보까지 포함하여 healthcheck 응답을 결정하도록 개선할 계획
    let (reporter, health_service) = health_reporter();
    reporter
        .set_service_status("", ServingStatus::Serving)
        .await;

    println!("starting gRPC server on {address}");
    Server::builder()
        .add_service(health_service)
        .serve_with_shutdown(address, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
