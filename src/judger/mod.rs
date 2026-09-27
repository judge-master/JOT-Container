mod checker;
mod executor;
mod problem;

use checker::Checker;
use executor::Executor;

pub struct Grader<C: Checker, E: Executor> {
    pub checker: C,
    pub executor: E,
}

impl<C: Checker, E: Executor> Grader<C, E> {
    pub fn new(checker: C, executor: E) -> Self {
        Self { checker, executor }
    }
}