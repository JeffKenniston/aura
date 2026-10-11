pub mod firecracker;
pub mod proxy;
pub mod wasm;

pub use firecracker::{should_skip_kvm_validation, validate_kvm_at_path, validate_kvm_support, KvmError};

pub fn init_wasmtime() -> Result<(), Box<dyn std::error::Error>> {
    println!("Embedding wasmtime engine with fuel metering and pooling allocators...");
    let _engine = wasm::create_engine()?;
    println!("Wasmtime engine embedded successfully.");
    Ok(())
}

pub fn init_firecracker() -> Result<(), Box<dyn std::error::Error>> {
    println!("Preparing ephemeral Firecracker microVM KVM management via jailer...");
    firecracker::validate_kvm_support()?;
    println!("Firecracker KVM virtualization verified successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_firecracker() {
        std::env::set_var("AURA_SKIP_KVM_VALIDATION", "1");
        let result = init_firecracker();
        assert!(result.is_ok(), "init_firecracker should return Ok");
    }

    #[test]
    fn test_init_wasmtime() {
        let result = init_wasmtime();
        assert!(result.is_ok(), "init_wasmtime should return Ok");
    }
}
