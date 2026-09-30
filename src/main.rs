mod config;
mod grpc;
mod judge;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let address = config::grpc_address()?;

    // run gRPC server
    grpc::serve(address).await?;
    Ok(())
}
