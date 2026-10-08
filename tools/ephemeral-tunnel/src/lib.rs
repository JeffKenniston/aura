pub mod credentials;
pub mod spire;

pub use credentials::{Credentials, TokenMetadata};
pub use spire::EphemeralTunnelService;

// Generate WIT bindings for the aura:tunnel WebAssembly component
wit_bindgen::generate!({
    path: "wit",
    world: "tunnel",
});

struct EphemeralTunnelComponent;

impl exports::aura::tunnel::ephemeral::Guest for EphemeralTunnelComponent {
    fn mint_credentials(
        service: String,
    ) -> Result<exports::aura::tunnel::ephemeral::Credentials, String> {
        let tunnel = EphemeralTunnelService::default();
        let creds = tunnel.mint_credentials(&service)?;
        Ok(exports::aura::tunnel::ephemeral::Credentials {
            token: creds.token,
            expires_at: creds.expires_at,
        })
    }

    fn revoke_credentials(token: String) -> Result<(), String> {
        let tunnel = EphemeralTunnelService::default();
        tunnel.revoke_credentials(&token)
    }
}

#[cfg(target_arch = "wasm32")]
export!(EphemeralTunnelComponent);
