mod checker;
mod executor;

use checker::Checker;
use executor::{Executor, ExecuteError, ExecuteResult};
use std::sync::Arc;

// Arc is used for efficient caching between multiple threads
pub struct Testcase {
    pub input: Arc<str>,
    pub answer: Arc<str>,
}

pub struct Problem {
    pub id: String,
    pub memory_limit: u64,
    pub instruction_limit: u64,
    pub tests: Vec<Testcase>,
}


pub struct Grader<C: Checker, E: Executor> {
    pub checker: C,
    pub executor: E,
}

#[derive(Debug)]
pub enum Verdict {
    Accepted,
    WrongAnswer,
    CompilationError(Option<String>),
    RuntimeError(Option<String>),
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(u64),
    BuildArtifactLimitExceeded(u64),
    SystemError(Option<String>),
}
#[derive(Debug)]
pub struct GradeResult {
    pub verdict: Verdict,
    pub memory_used: u64,
    pub instruction_count: u64,
}

impl<C: Checker, E: Executor> Grader<C, E> {
    pub fn new(checker: C, executor: E) -> Self {
        Self { checker, executor }
    }
    pub async fn grade(&mut self, problem: Problem) -> GradeResult {
        let checker = &mut self.checker;
        let executor = &mut self.executor;

        let mut grade_result = GradeResult {
            verdict: Verdict::SystemError(Some("Unknown error".to_string())),
            memory_used: 0,
            instruction_count: 0,
        };

        for (idx, test) in problem.tests.iter().enumerate() {
            let res = executor.execute(&test.input, problem.memory_limit, problem.instruction_limit).await;
            match res {
                Ok(ExecuteResult {output, memory_used, instruction_count}) => {
                    grade_result.memory_used = grade_result.memory_used.max(memory_used);
                    grade_result.instruction_count = grade_result.instruction_count.max(instruction_count);

                    if !checker.check(&output, &test.answer).await.unwrap_or(false) {
                        grade_result.verdict = Verdict::WrongAnswer;
                        return grade_result;
                    }
                }
                Err(ExecuteError::TimeLimitExceeded(time)) => {
                    grade_result.verdict = Verdict::TimeLimitExceeded(time);
                    return grade_result;
                }
                Err(ExecuteError::MemoryLimitExceeded(memory)) => {
                    grade_result.verdict = Verdict::MemoryLimitExceeded(memory);
                    return grade_result;
                }
                Err(ExecuteError::RuntimeError { reason }) => {
                    grade_result.verdict = Verdict::RuntimeError(reason);
                    return grade_result;
                }
                Err(ExecuteError::CompilationError { message }) => {
                    grade_result.verdict = Verdict::CompilationError(message);
                    return grade_result;
                }
            }
        }

        grade_result.verdict = Verdict::Accepted;
        grade_result
    }
}

#[tokio::test]
async fn test_grader() {
    use checker::lcmp_checker::LcmpChecker;
    use executor::aplusb_executor::APlusBExecutor;
    use std::sync::Arc;

    let problem = Problem {
        id: "1".into(),
        tests: vec![
            Testcase {
                input: Arc::from("1 2"),
                answer: Arc::from("3"),
            },
            Testcase {
                input: Arc::from("100 200"),
                answer: Arc::from("300"),
            },
        ],
        memory_limit: 1024 * 1024,
        instruction_limit: 100000000,
    };

    let mut grader = Grader::new(LcmpChecker, APlusBExecutor);
    let result = grader.grade(problem).await;

    assert_eq!(matches!(result.verdict, Verdict::Accepted), true);
}

#[tokio::test]
async fn test_grader_instruction_limit() {
    use checker::lcmp_checker::LcmpChecker;
    use executor::aplusb_executor::APlusBExecutor;
    use std::sync::Arc;

    let problem = Problem {
        id: "1".into(),
        tests: vec![
            Testcase {
                input: Arc::from("1 2"),
                answer: Arc::from("3"),
            },
            Testcase {
                input: Arc::from("100 200"),
                answer: Arc::from("300"),
            },
        ],
        memory_limit: 1024 * 1024,
        instruction_limit: 1,
    };

    let mut grader = Grader::new(LcmpChecker, APlusBExecutor);
    let result = grader.grade(problem).await;

    if let Verdict::TimeLimitExceeded(_) = result.verdict {
        return;
    } else {
        panic!("Expected TimeLimitExceeded, got {:?}", result.verdict);
    }
}