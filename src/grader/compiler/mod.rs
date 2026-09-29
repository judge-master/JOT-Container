pub mod c;
pub mod cpp;

mod compile_executor;

use std::{path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};
use async_trait::async_trait;
use crate::grader::executor::Executor;

static BINARY_ID: AtomicU64 = AtomicU64::new(1);

#[async_trait]
pub trait Compiler {
    fn create_binary_path(&self) -> PathBuf {
        let build_dir = std::env::var("BUILD_DIR").expect("BUILD_DIR environment is needed");
        let build_dir = Path::new(&build_dir);

        build_dir.join(format!("{}.out", BINARY_ID.fetch_add(1, Ordering::Relaxed)))
    }

    // returns the path to the compiled binary if successful, or an error message if failed
    async fn compile(&mut self, source: &str) -> Result<(Box<dyn Executor>, String), String>;
}