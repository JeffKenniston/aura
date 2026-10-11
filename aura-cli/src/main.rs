fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Aura CLI");
    let rt = aura_core::runtime::init_tokio_pool(None)?;
    let workers = rt.metrics().num_workers();
    println!("Aura Tokio runtime pool initialized with {} worker threads", workers);
    Ok(())
}
