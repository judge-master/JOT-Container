use std::{env, net::SocketAddr};

const DEFAULT_GRPC_ADDRESS: &str = "0.0.0.0:50051";

pub fn grpc_address() -> Result<SocketAddr, Box<dyn std::error::Error>> {
    let address = env::var("GRPC_ADDR").unwrap_or_else(|_| DEFAULT_GRPC_ADDRESS.into());
    Ok(address.parse()?)
}
