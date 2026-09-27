mod checker;
mod executor;
mod problem;

use checker::Checker;
use executor::{Executor, ExecuteError, ExecuteResult};
use problem::Problem;

pub struct Grader<C: Checker, E: Executor> {
    pub checker: C,
    pub executor: E,
}

pub enum Verdict {
    Accepted,
    WrongAnswer,
    CompilationError(Option<String>),
    RuntimeError(Option<String>),
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(usize),
    BuildArtifactLimitExceeded(usize),
    SystemError(Option<String>),
}
pub struct GradeResult {
    pub verdict: Verdict,
    pub memory_used: usize,
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
            let res = executor.execute(&test.input, problem.memory_limit, problem.time_limit).await;
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
                Err(ExecuteError::CompileError { message }) => {
                    grade_result.verdict = Verdict::CompilationError(message);
                    return grade_result;
                }
            }
        }

        grade_result.verdict = Verdict::Accepted;
        grade_result
    }
}