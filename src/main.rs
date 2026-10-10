mod config;
mod grpc;
mod judge;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    config::init_config().map_err(|e| format!("Failed to load configuration: {}", e))?;
    let address = config::grpc_address();
    let judge_operator = judge::JudgeQueue::new(10);

    // run gRPC server
    grpc::serve(address, judge_operator).await?;
    Ok(())
}
