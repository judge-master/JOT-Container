mod config;
mod grpc;
mod judge;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    config::init_config().map_err(|e| format!("Failed to load configuration: {}", e))?;
    let address = config::grpc_address();

    // run gRPC server
    grpc::serve(address).await?;
    Ok(())
}
