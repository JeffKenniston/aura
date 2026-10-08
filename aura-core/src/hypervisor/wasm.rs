use crate::config::WasmConfig;
use crate::identity::{
    enforcer::{Capability, CapabilityEnforcer},
    Svid,
};
use wasmtime::{Config, Engine, OptLevel};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

/// Creates a wasmtime Engine configured for WASI 0.3 asynchronous execution (EXE-WASM-001 - 008).
/// Enforces Cranelift JIT optimization, strict fuel metering, and pooling allocators.
pub fn create_engine() -> Result<Engine, Box<dyn std::error::Error>> {
    let default_config = WasmConfig::default();
    create_engine_with_config(&default_config)
}

/// Creates a wasmtime Engine configured from declarative WasmConfig.
pub fn create_engine_with_config(
    config: &WasmConfig,
) -> Result<Engine, Box<dyn std::error::Error>> {
    let mut wasmtime_config = Config::new();

    // Cranelift JIT compilation optimization (EXE-WASM-002)
    match config.opt_level.as_str() {
        "none" => wasmtime_config.cranelift_opt_level(OptLevel::None),
        "size" => wasmtime_config.cranelift_opt_level(OptLevel::SpeedAndSize),
        _ => wasmtime_config.cranelift_opt_level(OptLevel::Speed),
    };

    // Asynchronous execution support for Tokio integration (EXE-WASM-003)
    wasmtime_config.async_support(true);

    // WASM Component Model support for WASI 0.3 (EXE-WASM-004)
    wasmtime_config.wasm_component_model(true);

    // Instruction fuel metering to deterministically terminate runaway executions (EXE-WASM-005)
    wasmtime_config.consume_fuel(true);

    // Pooling allocator for high concurrency and sub-millisecond instantiation (EXE-WASM-006)
    if config.pooling_allocator {
        wasmtime_config.allocation_strategy(wasmtime::InstanceAllocationStrategy::Pooling(
            wasmtime::PoolingAllocationConfig::default(),
        ));
    }

    Engine::new(&wasmtime_config).map_err(|e| e.into())
}

/// State container for WASI 0.3 Component Model execution.
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

/// Builds an asynchronous WASI Component Model linker (EXE-WASM-007).
pub fn create_linker(
    engine: &Engine,
) -> Result<wasmtime::component::Linker<WasmState>, Box<dyn std::error::Error>> {
    let mut linker = wasmtime::component::Linker::new(engine);

    // Map standard WASI Component Model interfaces asynchronously
    wasmtime_wasi::add_to_linker_async(&mut linker)?;

    Ok(linker)
}

/// Constructs the WASI state after verifying Zero-Trust capability authorization.
pub fn build_wasi_state(svid: Svid) -> Result<WasmState, Box<dyn std::error::Error>> {
    // Zero-Trust Enforcement: WASI component execution requires cryptographic capability validation
    CapabilityEnforcer::check(&svid, Capability::FileSystemRead)?;

    let mut builder = WasiCtxBuilder::new();
    builder.inherit_stdio();

    Ok(WasmState {
        svid: svid.id.clone(),
        ctx: builder.build(),
        table: ResourceTable::new(),
    })
}

/// Executes a WASI 0.3 WebAssembly component with fuel metering (EXE-WASM-001 - 008).
pub async fn execute_component(
    engine: &Engine,
    component_bytes: &[u8],
    svid: Svid,
    fuel_budget: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let component = wasmtime::component::Component::new(engine, component_bytes)?;
    let linker = create_linker(engine)?;
    let wasi_state = build_wasi_state(svid)?;
    let mut store = wasmtime::Store::new(engine, wasi_state);

    // Set fuel budget to prevent unbounded functional execution
    store.set_fuel(fuel_budget)?;

    // Instantiate the component asynchronously using the WASI 0.3 Component Model linker
    let instance = linker.instantiate_async(&mut store, &component).await?;

    // Invoke standard WASI CLI command entrypoint if exported
    if let Some(func) = instance.get_func(&mut store, "wasi:cli/run@0.2.0#run") {
        let typed_func = func.typed::<(), (Result<(), ()>,)>(&store)?;
        let _ = typed_func.call_async(&mut store, ()).await?;
    }

    Ok(())
}

/// Runs a standard WASM module task with a designated fuel budget and SVID authorization.
pub async fn run_wasm_task(
    engine: &Engine,
    module: &wasmtime::Module,
    fuel_budget: u64,
    svid: Svid,
) -> Result<(), Box<dyn std::error::Error>> {
    let wasi_state = build_wasi_state(svid)?;
    let mut store = wasmtime::Store::new(engine, wasi_state);
    store.set_fuel(fuel_budget)?;

    let _instance = wasmtime::Instance::new_async(&mut store, module, &[]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasmtime::{Module, Store};

    #[tokio::test]
    async fn test_wasi_component_execution() -> Result<(), Box<dyn std::error::Error>> {
        let engine = create_engine()?;

        // Define a minimal valid WASI 0.3 Component
        let wat = r#"
            (component)
        "#;

        let component_bytes = wat::parse_str(wat)?;

        let mock_svid = Svid::new("spiffe://aura.local/trusted/sandbox/component_test")
            .with_scopes(vec!["fs:read".to_string()]);

        // Ensure instantiation of a pure component functions via the async linker
        execute_component(&engine, &component_bytes, mock_svid, 10_000).await?;

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

        let mock_svid = Svid::new("spiffe://aura.local/trusted/sandbox/test");

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
        let mock_svid = Svid::new("spiffe://aura.local/agent/malicious");

        let result = CapabilityEnforcer::check(&mock_svid, Capability::FileSystemWrite);
        assert!(result.is_err());
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("Zero-Trust Violation"));
    }
}
