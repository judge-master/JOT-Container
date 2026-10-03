fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_prost_build::compile_protos(std::path::Path::new("proto/judge/v1/judge.proto"))?;
    Ok(())
}
