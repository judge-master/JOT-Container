pub mod lcmp_checker;

use async_trait::async_trait;

#[async_trait]
pub trait Checker: Send {
    async fn check(&mut self, output: &str, answer: &str) -> Result<bool, Box<dyn std::error::Error>>;
}

