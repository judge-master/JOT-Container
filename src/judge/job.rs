use crate::judge::{
    Problem,
    checker::lcmp_checker::LcmpChecker,
    compiler,
    grader::{self, Grader},
    testcase::{TestcaseFetcher, lru_fetcher::LruTestcaseFetcher},
};
use jot_proto::judge::v1::JudgeEvent;
use lazy_static::lazy_static;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

// 일단 임시로 static으로 해놨는데 나중에 config등을 이용해서 더 좋게 바꿀 수 있을 듯
lazy_static! {
    static ref TESTCASE_FETCHER: LruTestcaseFetcher = LruTestcaseFetcher::new();
}
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

        // preload를 여기서 호출할 수 있음
        // PROBLEM_FETCHER.preload(self.request.problem_id);

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

        let compiler_result = match compiler
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

        let Ok(testcases) = TESTCASE_FETCHER.get(self.request.problem_id).await else {
            let judge_result = jot_proto::judge::v1::JudgeResult {
                verdict: jot_proto::judge::v1::Verdict::SystemError as i32,
                error_message: Some("Failed to fetch testcases".into()),
                ..Default::default()
            };
            let event = JudgeEvent {
                request_id: self.request.request_id,
                payload: Some(jot_proto::judge::v1::judge_event::Payload::Result(
                    judge_result,
                )),
            };
            self.response_channel.send(event).await?;
            return Ok(());
        };
        let Some(testcases) = testcases else {
            let judge_result = jot_proto::judge::v1::JudgeResult {
                verdict: jot_proto::judge::v1::Verdict::SystemError as i32,
                error_message: Some("Testcases not found".into()),
                ..Default::default()
            };
            let event = JudgeEvent {
                request_id: self.request.request_id,
                payload: Some(jot_proto::judge::v1::judge_event::Payload::Result(
                    judge_result,
                )),
            };
            self.response_channel.send(event).await?;
            return Ok(());
        };

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

        let grader = Grader::new(LcmpChecker::new(), compiler_result.executor);

        let mut stream = grader.grade_stream(Problem {
            memory_limit: limits.memory_bytes,
            instruction_limit: limits.instruction_count,
            tests: testcases,
        });
        let grade_result = loop {
            let Some(data) = stream.recv().await else {
                self.response_channel
                    .send(JudgeEvent {
                        request_id: self.request.request_id,
                        payload: Some(jot_proto::judge::v1::judge_event::Payload::Result(
                            jot_proto::judge::v1::JudgeResult {
                                verdict: jot_proto::judge::v1::Verdict::SystemError as i32,
                                error_message: Some("Grader stream closed unexpectedly".into()),
                                ..Default::default()
                            },
                        )),
                    })
                    .await?;
                return Ok(());
            };
            match data {
                grader::GradeStreamData::Progress {
                    completed_cases,
                    total_cases,
                } => {
                    let event = JudgeEvent {
                        request_id: self.request.request_id,
                        payload: Some(jot_proto::judge::v1::judge_event::Payload::Progress(
                            jot_proto::judge::v1::JudgeProgress {
                                phase: jot_proto::judge::v1::JudgePhase::Running as i32,
                                completed_cases,
                                total_cases,
                            },
                        )),
                    };
                    self.response_channel.send(event).await?;
                }
                grader::GradeStreamData::Result(grade_result) => {
                    break grade_result;
                }
            }
        };

        let (verdict, error) = match grade_result.verdict {
            grader::Verdict::Accepted => (jot_proto::judge::v1::Verdict::Accepted, None),
            grader::Verdict::WrongAnswer => (jot_proto::judge::v1::Verdict::WrongAnswer, None),
            grader::Verdict::TimeLimitExceeded(time) => (
                jot_proto::judge::v1::Verdict::TimeLimitExceeded,
                Some(format!("Time limit exceeded: {} ms", time)),
            ),
            grader::Verdict::MemoryLimitExceeded(memory) => (
                jot_proto::judge::v1::Verdict::MemoryLimitExceeded,
                Some(format!("Memory limit exceeded: {} bytes", memory)),
            ),
            grader::Verdict::RuntimeError(e) => (
                jot_proto::judge::v1::Verdict::RuntimeError,
                Some(format!("Runtime error: {:?}", e)),
            ),
            grader::Verdict::CompilationError(e) => (
                jot_proto::judge::v1::Verdict::CompilationError,
                Some(format!("Compilation error: {:?}", e)),
            ),
            grader::Verdict::SystemError(e) => (
                jot_proto::judge::v1::Verdict::SystemError,
                Some(format!("System error: {:?}", e)),
            ),
            grader::Verdict::BuildArtifactLimitExceeded(e) => (
                jot_proto::judge::v1::Verdict::BuildArtifactLimitExceeded,
                Some(format!("Build artifact limit exceeded: {:?}", e)),
            ),
        };

        let judge_result = jot_proto::judge::v1::JudgeResult {
            verdict: verdict as i32,
            compiler_diagnostics: compiler_result.diagnostics,
            error_message: error,
            max_memory_bytes: Some(grade_result.memory_used),
            max_instruction_count: Some(grade_result.instruction_count),
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
