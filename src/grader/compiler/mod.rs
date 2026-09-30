pub mod c;
pub mod cpp;

mod compile_executor;

use async_trait::async_trait;
use crate::grader::executor::Executor;

pub struct CompilerResult {
    pub executor: Box<dyn Executor>,
    pub diagnostics: String,
}

#[async_trait]
pub trait Compiler {
    // Returns an executor holding the compiled Wasm bytes and any compiler diagnostics.
    async fn compile(&mut self, source: &str) -> Result<CompilerResult, String>;
}