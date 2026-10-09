pub mod c;
pub mod cpp;

mod compile_executor;

use crate::judge::executor::Executor;
use async_trait::async_trait;

pub fn get_compiler(language: jot_proto::judge::v1::Language) -> Box<dyn Compiler> {
    match language {
        jot_proto::judge::v1::Language::C => Box::new(c::CCompiler {}),
        jot_proto::judge::v1::Language::Cpp => Box::new(cpp::CppCompiler {}),
        _ => panic!("Unsupported language"),
    }
}

pub struct CompilerResult {
    pub executor: Box<dyn Executor>,
    pub diagnostics: String,
}

#[derive(Debug)]
pub enum CompilerError {
    TimeLimitExceeded(u64),
    MemoryLimitExceeded(u64),
    BuildArtifactSizeLimitExceeded(u64),
    CompilationError(String),
    RuntimeError(String),
}

impl CompilerError {
    pub fn to_string(&self) -> String {
        match self {
            CompilerError::TimeLimitExceeded(limit) => format!("Time limit exceeded: {} ms", limit),
            CompilerError::MemoryLimitExceeded(limit) => {
                format!("Memory limit exceeded: {} bytes", limit)
            }
            CompilerError::BuildArtifactSizeLimitExceeded(limit) => {
                format!("Build artifact size limit exceeded: {} bytes", limit)
            }
            CompilerError::CompilationError(msg) => format!("Compilation error: {}", msg),
            CompilerError::RuntimeError(msg) => format!("Runtime error: {}", msg),
        }
    }
}

pub struct CompilerResourceLimits {
    pub memory_limit_byte: u64,
    pub time_limit_ms: u64,
    pub build_artifact_size_limit_byte: u64,
}

#[async_trait]
pub trait Compiler: Send {
    // Returns an executor holding the compiled Wasm bytes and any compiler diagnostics.
    async fn compile(
        &self,
        source: &str,
        limits: CompilerResourceLimits,
    ) -> Result<CompilerResult, CompilerError>;
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::{Compiler, CompilerError, CompilerResourceLimits};

    pub fn default_limits() -> CompilerResourceLimits {
        CompilerResourceLimits {
            memory_limit_byte: 64 * 1024 * 1024,
            time_limit_ms: 10_000,
            build_artifact_size_limit_byte: 10 * 1024 * 1024,
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub enum ExpectedError {
        TimeLimitExceeded(u64),
        BuildArtifactSizeLimitExceeded(u64),
    }

    pub async fn assert_compiles(compiler: &mut impl Compiler, source: &str) {
        if let Err(err) = compiler.compile(source, default_limits()).await {
            panic!("Compilation failed: {err:?}");
        }
    }

    pub async fn assert_compile_error(
        compiler: &mut impl Compiler,
        source: &str,
        limits: CompilerResourceLimits,
        expected: ExpectedError,
    ) {
        let result = compiler.compile(source, limits).await;
        let matches_expected = match (&result, expected) {
            (
                Err(CompilerError::TimeLimitExceeded(actual)),
                ExpectedError::TimeLimitExceeded(expected),
            ) => *actual == expected,
            (
                Err(CompilerError::BuildArtifactSizeLimitExceeded(actual)),
                ExpectedError::BuildArtifactSizeLimitExceeded(expected),
            ) => *actual == expected,
            _ => false,
        };
        assert!(
            matches_expected,
            "Expected {expected:?}, got {}",
            match result {
                Ok(_) => "success".to_string(),
                Err(err) => format!("{err:?}"),
            }
        );
    }
}
