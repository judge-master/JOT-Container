use std::{path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};
use async_trait::async_trait;

static BINARY_ID: AtomicU64 = AtomicU64::new(1);

#[async_trait]
pub trait Compiler {
    fn create_binary_path(&self) -> PathBuf {
        let mut path = PathBuf::from("build");
        path.push(format!("{}.out", BINARY_ID.fetch_add(1, Ordering::Relaxed)));
        path
    }

    // returns the path to the compiled binary if successful, or an error message if failed
    async fn compile(&mut self, source: &str) -> Result<PathBuf, String>;
}

pub struct CCompiler;
#[async_trait]
impl Compiler for CCompiler {
    async fn compile(&mut self, source: &str) -> Result<PathBuf, String> {
        let binary_path = self.create_binary_path();
        
        todo!()
    }
}