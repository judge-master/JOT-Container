use std::{net::SocketAddr, pin::Pin};

use crate::generated::jot::judge_event::Payload;
use crate::generated::jot::judge_service_server::{JudgeService, JudgeServiceServer};
use crate::generated::jot::{JudgeEvent, JudgeRequest, JudgeResult, Verdict};
use tokio_stream::Stream;
use tonic::async_trait;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
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
        .add_service(JudgeServiceServer::new(Service))
        .serve_with_shutdown(address, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

pub struct Service;
#[tonic::async_trait]
impl JudgeService for Service {
    type JudgeStream = Pin<Box<dyn Stream<Item = Result<JudgeEvent, Status>> + Send>>;
    async fn judge(
        &self,
        request: Request<JudgeRequest>,
    ) -> Result<tonic::Response<Self::JudgeStream>, tonic::Status> {
        println!("Request: {:?}", request.into_inner());
        let mut events = Vec::new();
        events.push(Ok(JudgeEvent {
            request_id: 1,
            payload: Some(Payload::Result(JudgeResult {
                verdict: Verdict::Accepted.into(),
                max_instruction_count: None,
                max_memory_bytes: None,
                compiler_diagnostics: "Hello world!".into(),
                error_message: None,
            })),
        }));
        let output: Self::JudgeStream = Box::pin(tokio_stream::iter(events));
        Ok(Response::new(output))
    }
}
