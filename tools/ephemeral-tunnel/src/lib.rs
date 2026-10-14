use std::sync::OnceLock;

pub mod credentials;
pub mod spire;

pub use credentials::{Credentials, TokenMetadata};
pub use spire::EphemeralTunnelService;

static GLOBAL_SERVICE: OnceLock<EphemeralTunnelService> = OnceLock::new();

/// Returns a reference to the global thread-safe `EphemeralTunnelService` singleton.
pub fn global_service() -> &'static EphemeralTunnelService {
    GLOBAL_SERVICE.get_or_init(EphemeralTunnelService::default)
}

/// Allows explicit initialization of the global service before first use.
pub fn init_global_service(service: EphemeralTunnelService) -> Result<(), EphemeralTunnelService> {
    GLOBAL_SERVICE.set(service)
}

// Generate WIT bindings for the aura:tunnel WebAssembly component
wit_bindgen::generate!({
    path: "../../aura/runtimes/wasm/wit/aura-tunnel.wit",
    world: "tunnel",
});

pub struct EphemeralTunnelComponent;

impl exports::aura::tunnel::ephemeral::Guest for EphemeralTunnelComponent {
    fn mint_credentials(
        service: String,
    ) -> Result<exports::aura::tunnel::ephemeral::Credentials, String> {
        let tunnel = global_service();
        let creds = tunnel.mint_credentials(&service)?;
        Ok(exports::aura::tunnel::ephemeral::Credentials {
            token: creds.token,
            expires_at: creds.expires_at,
        })
    }

    fn revoke_credentials(token: String) -> Result<(), String> {
        let tunnel = global_service();
        tunnel.revoke_credentials(&token)
    }
}

#[cfg(target_arch = "wasm32")]
export!(EphemeralTunnelComponent);

