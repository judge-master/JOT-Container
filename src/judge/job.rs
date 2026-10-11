use jot_proto::judge::v1::JudgeEvent;

use crate::judge::compiler;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub struct JudgeJob {
    request: jot_proto::judge::v1::JudgeRequest,
    response_channel: tokio::sync::mpsc::Sender<jot_proto::judge::v1::JudgeEvent>,
}

impl JudgeJob {
    pub async fn new(
        request: jot_proto::judge::v1::JudgeRequest,
        response_channel: tokio::sync::mpsc::Sender<jot_proto::judge::v1::JudgeEvent>,
    ) -> Result<Self, String> {
        // send JudgePhase::RECEIVED event to the response channel
        let event = JudgeEvent {
            request_id: request.request_id,
            payload: Some(jot_proto::judge::v1::judge_event::Payload::Progress(
                jot_proto::judge::v1::JudgeProgress {
                    phase: jot_proto::judge::v1::JudgePhase::Received as i32,
                    completed_cases: 0,
                    total_cases: 0, // TODO: set total_cases to the number of test cases in the request
                },
            )),
        };
        response_channel
            .send(event)
            .await
            .map_err(|e| format!("Failed to send JudgePhase::RECEIVED event: {}", e))?;

        Ok(Self {
            request,
            response_channel,
        })
    }

    pub async fn run(self) -> Result<(), Error> {
        let limits = self
            .request
            .limits
            .as_ref()
            .ok_or("Resource limits not provided in the request")?;

        // 컴파일 시작 이벤트 발송
        self.response_channel
            .send(JudgeEvent {
                request_id: self.request.request_id,
                payload: Some(jot_proto::judge::v1::judge_event::Payload::Progress(
                    jot_proto::judge::v1::JudgeProgress {
                        phase: jot_proto::judge::v1::JudgePhase::Compiling as i32,
                        completed_cases: 0,
                        total_cases: 0, // TODO: set total_cases to the number of test cases in the request
                    },
                )),
            })
            .await?;

        // 컴파일 진행
        let compiler = compiler::get_compiler(self.request.language());
        // Compile the source code

        let mut compiler_result = match compiler
            .compile(
                &self.request.source_code,
                compiler::CompilerResourceLimits {
                    memory_limit_byte: 1000 * 1024 * 1024, // 1GB
                    time_limit_ms: 1000 * 1000,
                    build_artifact_size_limit_byte: limits.build_artifact_bytes,
                },
            )
            .await
        {
            Ok(compiler_result) => compiler_result,
            Err(e) => {
                let judge_result = jot_proto::judge::v1::JudgeResult {
                    verdict: jot_proto::judge::v1::Verdict::CompilationError as i32,
                    compiler_diagnostics: e.to_string(),
                    ..Default::default()
                };
                // 컴파일 에러 이벤트 발송
                let event = JudgeEvent {
                    request_id: self.request.request_id,
                    payload: Some(jot_proto::judge::v1::judge_event::Payload::Result(
                        judge_result,
                    )),
                };
                self.response_channel.send(event).await?;
                return Ok(());
            }
        };

        // 테스트 케이스 다운로드 시작 이벤트 발송
        self.response_channel
            .send(JudgeEvent {
                request_id: self.request.request_id,
                payload: Some(jot_proto::judge::v1::judge_event::Payload::Progress(
                    jot_proto::judge::v1::JudgeProgress {
                        phase: jot_proto::judge::v1::JudgePhase::FetchingCases as i32,
                        completed_cases: 0,
                        total_cases: 0, // TODO: set total_cases to the number of test cases in the request
                    },
                )),
            })
            .await?;

        // TODO: 테스트 케이스 다운로드/캐싱 로직 구현

        // 실행 중 이벤트 발송
        self.response_channel
            .send(JudgeEvent {
                request_id: self.request.request_id,
                payload: Some(jot_proto::judge::v1::judge_event::Payload::Progress(
                    jot_proto::judge::v1::JudgeProgress {
                        phase: jot_proto::judge::v1::JudgePhase::Running as i32,
                        completed_cases: 0,
                        total_cases: 0, // TODO: set total_cases to the number of test cases in the request
                    },
                )),
            })
            .await?;

        // TODO: 테스트 케이스 적용 구현
        let _ = compiler_result
            .executor
            .execute("1 2", limits.memory_bytes, limits.instruction_count)
            .await;

        // 결과 이벤트 발송

        let judge_result = jot_proto::judge::v1::JudgeResult {
            verdict: jot_proto::judge::v1::Verdict::Accepted as i32,
            compiler_diagnostics: compiler_result.diagnostics,
            ..Default::default()
        };
        let event = JudgeEvent {
            request_id: self.request.request_id,
            payload: Some(jot_proto::judge::v1::judge_event::Payload::Result(
                judge_result,
            )),
        };
        self.response_channel.send(event).await?;
        Ok(())
    }
}
