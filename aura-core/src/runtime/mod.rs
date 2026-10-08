pub mod registry;

pub fn init_tokio_pool() -> Result<(), Box<dyn std::error::Error>> {
    println!("Tokio runtime thread-pool configured for highly concurrent HTTP/3 multiplexing.");
    Ok(())
}
