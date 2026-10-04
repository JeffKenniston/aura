pub mod firecracker;
pub mod tools;
pub mod wasm;
pub mod wasm_tools;

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
