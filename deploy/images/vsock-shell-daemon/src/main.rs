mod server;
mod session;

use server::{run_tcp_listener, run_vsock_listener};
use session::SessionManager;
use std::sync::{Arc, Mutex};

fn print_usage() {
    println!("Aura Persistent Shell Daemon (TOOL-BASH-002)");
    println!("Usage: vsock-shell-daemon [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --port <PORT>        Vsock/TCP listening port (default: 5250)");
    println!("  --tcp                Listen on TCP 0.0.0.0 instead of AF_VSOCK (host testing)");
    println!("  --tcp-listen <PORT>  Listen on TCP port");
    println!("  --help               Display this help message");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let mut port: u32 = 5250;
    let mut use_tcp = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                return Ok(());
            }
            "--tcp" => {
                use_tcp = true;
            }
            "--port" | "-p" => {
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(5250);
                    i += 1;
                }
            }
            "--tcp-listen" => {
                use_tcp = true;
                if i + 1 < args.len() {
                    port = args[i + 1].parse().unwrap_or(5250);
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    let manager = Arc::new(Mutex::new(SessionManager::new()));

    if use_tcp {
        println!("[vsock-shell-daemon] Launching in TCP test mode on port {}", port);
        run_tcp_listener(port as u16, manager)?;
    } else {
        println!("[vsock-shell-daemon] Launching in AF_VSOCK mode on port {}", port);
        run_vsock_listener(port, manager)?;
    }

    Ok(())
}
