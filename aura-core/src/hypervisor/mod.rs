pub mod firecracker;
pub mod proxy;
pub mod wasm;

pub fn init_wasmtime() -> Result<(), Box<dyn std::error::Error>> {
    println!("Embedding wasmtime engine with fuel metering and pooling allocators...");
    let _engine = wasm::create_engine()?;
    println!("Wasmtime engine embedded successfully.");
    Ok(())
}

pub fn init_firecracker() -> Result<(), Box<dyn std::error::Error>> {
    println!("Preparing ephemeral Firecracker microVM KVM management via jailer...");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_firecracker() {
        let result = init_firecracker();
        assert!(result.is_ok(), "init_firecracker should return Ok");
    }

    #[test]
    fn test_init_wasmtime() {
        let result = init_wasmtime();
        assert!(result.is_ok(), "init_wasmtime should return Ok");
    }
}
