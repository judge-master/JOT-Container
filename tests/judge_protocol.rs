use std::{net::TcpListener, time::Duration};

use jot_proto::judge::v1::{
    JudgeEvent, JudgeRequest, JudgeResult, Language, ResourceLimits, Verdict, judge_event::Payload,
    judge_service_client::JudgeServiceClient,
};
use tokio::process::{Child, Command};
use tonic::transport::{Channel, Endpoint};

fn request(request_id: i64) -> JudgeRequest {
    JudgeRequest {
        request_id,
        language: Language::Cpp as i32,
        source_code: r#"#include <iostream>
int main() {
    long long a, b;
    std::cin >> a >> b;
    std::cout << a + b << '\n';
}
"#
        .into(),
        limits: Some(ResourceLimits {
            compile_time_ms: 10_000,
            instruction_count: 100_000_000,
            memory_bytes: 64 * 1024 * 1024,
            build_artifact_bytes: 16 * 1024 * 1024,
        }),
    }
}

async fn start_server() -> (Child, JudgeServiceClient<Channel>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);

    let mut server = Command::new(env!("CARGO_BIN_EXE_JOT-Container"))
        .env("GRPC_ADDR", address.to_string())
        .kill_on_drop(true)
        .spawn()
        .expect("failed to spawn gRPC server");
    let endpoint = Endpoint::from_shared(format!("http://{address}"))
        .unwrap()
        .connect_timeout(Duration::from_secs(1));
    let channel = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            assert!(
                server.try_wait().unwrap().is_none(),
                "gRPC server exited before accepting connections"
            );
            match endpoint.connect().await {
                Ok(channel) => break channel,
                Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    })
    .await
    .expect("gRPC server did not start");
    (server, JudgeServiceClient::new(channel))
}

async fn judge_events(
    client: &mut JudgeServiceClient<Channel>,
    request: JudgeRequest,
) -> Vec<JudgeEvent> {
    tokio::time::timeout(Duration::from_secs(30), async {
        let mut stream = client
            .judge(request)
            .await
            .expect("Judge RPC failed")
            .into_inner();
        let mut events = Vec::new();
        while let Some(event) = stream.message().await.expect("Judge stream failed") {
            events.push(event);
        }
        events
    })
    .await
    .expect("Judge stream did not finish")
}

fn terminal_result(events: &[JudgeEvent]) -> &JudgeResult {
    let (last, progress) = events.split_last().expect("Judge stream was empty");
    for event in progress {
        assert!(
            matches!(&event.payload, Some(Payload::Progress(_))),
            "only progress events may precede the terminal result: {event:?}"
        );
    }
    match &last.payload {
        Some(Payload::Result(result)) => result,
        payload => panic!("expected a terminal result, got {payload:?}"),
    }
}

#[tokio::test]
async fn judge_rpc_stream_returns_terminal_result() {
    let (mut server, mut client) = start_server().await;
    let events = judge_events(&mut client, request(1)).await;
    assert!(events.iter().all(|event| event.request_id == 1));
    assert_eq!(terminal_result(&events).verdict(), Verdict::Accepted);
    server.kill().await.unwrap();
}
