pub mod enforcer;

use spiffe::workload_api::client::WorkloadApiClient;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub struct Svid {
    pub id: String,
    pub scopes: HashSet<String>,
}

pub async fn fetch_svid() -> Result<Svid, Box<dyn std::error::Error>> {
    println!("Fetching SPIFFE Verifiable Identity Document (SVID) via Workload API...");

    let socket_path = std::env::var("SPIFFE_ENDPOINT_SOCKET")
        .unwrap_or_else(|_| "unix:///run/spire/sockets/agent.sock".to_string());

    // Connect to the Workload API.
    // Fallback to a mock SVID during test executions where SPIRE isn't running.
    match WorkloadApiClient::connect_to(&socket_path).await {
        Ok(client) => {
            let x509_svid = client.fetch_x509_svid().await?;
            let id = x509_svid.spiffe_id().to_string();
            println!("Successfully attested workload identity: {}", id);

            // For now, mock the SPIRE selector scope parsing based on the tenant.
            let mut scopes = HashSet::new();
            if id.contains("trusted") {
                scopes.insert("fs:read".to_string());
                scopes.insert("fs:write".to_string());
                scopes.insert("net:bind".to_string());
            } else {
                scopes.insert("fs:read".to_string());
            }

            Ok(Svid { id, scopes })
        }
        Err(e) => {
            println!(
                "Warning: Could not connect to SPIRE agent at {}. Error: {}",
                socket_path, e
            );
            println!("Warning: Falling back to mocked identity for testing...");

            let mut scopes = HashSet::new();
            scopes.insert("fs:read".to_string());
            scopes.insert("fs:write".to_string());
            scopes.insert("net:bind".to_string());

            Ok(Svid {
                id: "spiffe://aura.local/host".to_string(),
                scopes,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_fetch_svid_fallback() {
        // Save the old socket environment variable to restore it after the test
        let old_socket = std::env::var("SPIFFE_ENDPOINT_SOCKET").ok();

        // Force the connection to fail by providing a non-existent socket
        std::env::set_var("SPIFFE_ENDPOINT_SOCKET", "unix:///tmp/nonexistent_spire.sock");

        let svid_result = fetch_svid().await;
        assert!(svid_result.is_ok());

        let svid = svid_result.unwrap();
        assert_eq!(svid.id, "spiffe://aura.local/host");
        assert!(svid.scopes.contains("fs:read"));
        assert!(svid.scopes.contains("fs:write"));
        assert!(svid.scopes.contains("net:bind"));

        // Restore the old socket environment variable
        if let Some(socket) = old_socket {
            std::env::set_var("SPIFFE_ENDPOINT_SOCKET", socket);
        } else {
            std::env::remove_var("SPIFFE_ENDPOINT_SOCKET");
        }
    }
}
