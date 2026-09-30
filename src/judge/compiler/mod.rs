pub mod c;
pub mod cpp;

mod compile_executor;

use crate::judge::executor::Executor;
use async_trait::async_trait;

pub struct CompilerResult {
    pub executor: Box<dyn Executor>,
    pub diagnostics: String,
}

pub struct CompilerResourceLimits {
    pub memory_limit_byte: u64,
    pub time_limit_ms: u64,
    pub build_artifact_size_limit_byte: u64,
}

#[async_trait]
pub trait Compiler {
    // Returns an executor holding the compiled Wasm bytes and any compiler diagnostics.
    async fn compile(
        &mut self,
        source: &str,
        limits: CompilerResourceLimits,
    ) -> Result<CompilerResult, String>;
}
