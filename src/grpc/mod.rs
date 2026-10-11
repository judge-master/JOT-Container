use std::{net::SocketAddr, pin::Pin};

use jot_proto::judge::v1::judge_service_server::{JudgeService, JudgeServiceServer};
use jot_proto::judge::v1::{JudgeEvent, JudgeRequest};

use tokio_stream::StreamExt;

use tokio_stream::Stream;
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Server;
use tonic::{Request, Response, Status};
use tonic_health::{ServingStatus, server::health_reporter};

use crate::judge::JudgeQueue;

pub async fn serve(
    address: SocketAddr,
    judge_queue: JudgeQueue,
) -> Result<(), tonic::transport::Error> {
    // 일단 health_reporter를 사용하여 gRPC 서버의 healthchek 응답을 결정하고, 나중에 컴파일러나 Wasm 런타임 환경에 대한 정보까지 포함하여 healthcheck 응답을 결정하도록 개선할 계획
    let (reporter, health_service) = health_reporter();
    reporter
        .set_service_status("", ServingStatus::Serving)
        .await;

    println!("starting gRPC server on {address}");
    Server::builder()
        .add_service(health_service)
        .add_service(JudgeServiceServer::new(Service {
            judge_queue: judge_queue,
        }))
        .serve_with_shutdown(address, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}

pub struct Service {
    judge_queue: JudgeQueue,
}

#[tonic::async_trait]
impl JudgeService for Service {
    type JudgeStream = Pin<Box<dyn Stream<Item = Result<JudgeEvent, Status>> + Send>>;
    async fn judge(
        &self,
        request: Request<JudgeRequest>,
    ) -> Result<tonic::Response<Self::JudgeStream>, tonic::Status> {
        let req_payload = request.into_inner();

        let event_receiver = self
            .judge_queue
            .submit(req_payload)
            .await
            .map_err(Status::unavailable)?;
        let judge_stream = ReceiverStream::new(event_receiver).map(Ok::<JudgeEvent, Status>);

        Ok(Response::new(Box::pin(judge_stream)))
    }
}
