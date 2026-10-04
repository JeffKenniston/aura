use wasmtime::{Config, Engine, OptLevel};

pub fn create_engine() -> Result<Engine, Box<dyn std::error::Error>> {
    let mut config = Config::new();

    // Cranelift JIT compilation optimization
    config.cranelift_opt_level(OptLevel::Speed);

    // Asynchronous execution support (Tokio integration)
    config.async_support(true);

    // WASM Component Model support (WASI 0.3)
    config.wasm_component_model(true);

    // Instruction fuel metering (protection against infinite loops)
    config.consume_fuel(true);

    // Pooling allocators for high concurrency (optimizing memory footprint)
    config.allocation_strategy(wasmtime::InstanceAllocationStrategy::Pooling(
        wasmtime::PoolingAllocationConfig::default(),
    ));

    Engine::new(&config).map_err(|e| e.into())
}

use crate::identity::{
    enforcer::{Capability, CapabilityEnforcer},
    Svid,
};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

pub struct WasmState {
    pub svid: String,
    pub ctx: WasiCtx,
    pub table: ResourceTable,
}

impl WasiView for WasmState {
    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }

    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.ctx
    }
}

pub fn create_linker(
    engine: &Engine,
) -> Result<wasmtime::component::Linker<WasmState>, Box<dyn std::error::Error>> {
    let mut linker = wasmtime::component::Linker::new(engine);
    // Map standard WASI Component Model interfaces asynchronously
    wasmtime_wasi::add_to_linker_async(&mut linker)?;

    // Bind host tools
    crate::hypervisor::wasm_tools::bind_to_linker(&mut linker)?;

    Ok(linker)
}

pub fn build_wasi_state(svid: Svid) -> Result<WasmState, Box<dyn std::error::Error>> {
    // Zero-Trust Enforcement: WASI component execution requires cryptographic capabilities
    CapabilityEnforcer::check(&svid, Capability::FileSystemRead)?;

    let mut builder = WasiCtxBuilder::new();
    builder.inherit_stdio();

    Ok(WasmState {
        svid: svid.id.clone(),
        ctx: builder.build(),
        table: ResourceTable::new(),
    })
}

pub async fn execute_component(
    engine: &Engine,
    component_bytes: &[u8],
    svid: Svid,
) -> Result<(), Box<dyn std::error::Error>> {
    let component = wasmtime::component::Component::new(engine, component_bytes)?;
    let linker = create_linker(engine)?;
    let wasi_state = build_wasi_state(svid)?;
    let mut store = wasmtime::Store::new(engine, wasi_state);

    // Add default execution fuel to prevent unbounded functional tasks
    store.set_fuel(10_000)?;

    // Instantiate the component asynchronously using the WASI 0.3 Component Model linker
    let instance = linker.instantiate_async(&mut store, &component).await?;

    // Attempt to invoke the standard WASI command run function if it is exported
    if let Some(func) = instance.get_func(&mut store, "wasi:cli/run@0.2.0#run") {
        let typed_func = func.typed::<(), (Result<(), ()>,)>(&store)?;
        let _ = typed_func.call_async(&mut store, ()).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use wasmtime::{Module, Store};

    #[tokio::test]
    async fn test_wasi_component_execution() -> Result<(), Box<dyn std::error::Error>> {
        let engine = create_engine()?;

        // Define a minimal valid WASI 0.3 Component
        let wat = r#"
            (component)
        "#;

        let component_bytes = wat::parse_str(wat)?;

        let mut scopes = HashSet::new();
        scopes.insert("fs:read".to_string());
        let mock_svid = Svid {
            id: "spiffe://aura.local/sandbox/component_test".to_string(),
            scopes,
        };

        // Ensure instantiation of a pure component functions via the async linker
        execute_component(&engine, &component_bytes, mock_svid).await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_fuel_metering_trap() -> Result<(), Box<dyn std::error::Error>> {
        let engine = create_engine()?;

        let wat = r#"
            (module
                (func $loop (export "loop")
                    loop
                        br 0
                    end
                )
            )
        "#;

        let wasm_bytes = wat::parse_str(wat)?;
        let module = Module::new(&engine, &wasm_bytes)?;

        // Mock a privileged SVID capable of launching WASM Sandboxes
        let mut scopes = HashSet::new();
        scopes.insert("fs:read".to_string());

        let mock_svid = Svid {
            id: "spiffe://aura.local/sandbox/test".to_string(),
            scopes,
        };

        let wasi_state = build_wasi_state(mock_svid)?;
        let mut store = Store::new(&engine, wasi_state);
        store.set_fuel(100)?;

        let instance = wasmtime::Instance::new_async(&mut store, &module, &[]).await?;
        let loop_func = instance.get_typed_func::<(), ()>(&mut store, "loop")?;

        let result = loop_func.call_async(&mut store, ()).await;

        assert!(
            result.is_err(),
            "Expected execution to fail due to out of fuel"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{:?}", err).contains("fuel"),
            "Unexpected error: {:?}",
            err
        );

        Ok(())
    }

    #[test]
    fn test_wasi_capability_denied() {
        // Test an unprivileged SVID
        let mock_svid = Svid {
            id: "spiffe://aura.local/agent/malicious".to_string(),
            scopes: HashSet::new(),
        };

        let result = build_wasi_state(mock_svid);
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Zero-Trust Violation"));
    }
}
