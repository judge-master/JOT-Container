use std::{net::SocketAddr, pin::Pin};

use jot_proto::judge::v1::judge_event::Payload;
use jot_proto::judge::v1::judge_service_server::{JudgeService, JudgeServiceServer};
use jot_proto::judge::v1::{
    JudgeEvent, JudgePhase, JudgeProgress, JudgeRequest, JudgeResult, Verdict,
};

use tokio::sync::mpsc;
use tokio_stream::Stream;
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
        let req_payload = request.into_inner();

        let (tx, rx) = mpsc::channel::<Result<JudgeEvent, Status>>(8);

        tokio::spawn(async move {
            // Simulate some async work
            // TODO: 실제로는 컴파일러나 Wasm 런타임 환경에서 코드를 실행하고, 그 결과를 이벤트로 만들어서 보내야 함
            let total_cases = 9;
            for completed_cases in 1..=total_cases {
                let event = JudgeEvent {
                    request_id: req_payload.request_id,
                    payload: Some(Payload::Progress(JudgeProgress {
                        phase: JudgePhase::Running.into(),
                        completed_cases,
                        total_cases,
                    })),
                };
                if let Err(e) = tx.send(Ok(event)).await {
                    eprintln!("Failed to send event: {}", e);
                    return;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await; // Simulate some delay(50ms) between events
            }
            let event = JudgeEvent {
                request_id: req_payload.request_id,
                payload: Some(Payload::Result(JudgeResult {
                    verdict: Verdict::Accepted.into(),
                    max_instruction_count: Some(1000),
                    max_memory_bytes: Some(1024),
                    compiler_diagnostics: String::new(),
                    error_message: None,
                })),
            };
            if let Err(e) = tx.send(Ok(event)).await {
                eprintln!("Failed to send terminal result: {}", e);
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}
