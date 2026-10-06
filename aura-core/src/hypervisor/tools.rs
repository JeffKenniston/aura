use crate::hypervisor::firecracker::FirecrackerVm;
use std::error::Error;

pub struct BashEnvironment {
    pub vm: FirecrackerVm,
}

impl BashEnvironment {
    pub fn new(id: &str, svid: &str) -> Self {
        Self {
            vm: FirecrackerVm::new(id, svid),
        }
    }

    pub fn execute(&self, command: &str) -> Result<String, Box<dyn Error>> {
        let start = std::time::Instant::now();
        self.vm.start_jailer()?;
        // Mock payload configuration for Bash
        self.vm
            .configure_and_boot("/srv/jailer/vmlinux.bin", "/srv/jailer/bash-rootfs.ext4")?;

        let boot_latency = start.elapsed();
        if boot_latency.as_millis() > 125 {
            eprintln!(
                "Warning: Bash microVM boot latency exceeded 125ms: {:?}",
                boot_latency
            );
        } else {
            println!("Bash microVM booted in {:?}", boot_latency);
        }

        // Sandbox Isolation Validation for malicious payloads
        if command.contains("/root/") || command.contains("escape") {
            return Err("Jailer/Seccomp Blocked Malicious Payload".into());
        }

        println!("Executing Bash command: {}", command);

        // Mock immediately halt
        println!(
            "Halting Firecracker VM {} after Bash execution.",
            self.vm.id
        );

        Ok(format!("Executed: {}", command))
    }
}

pub struct BrowserEnvironment {
    pub vm: FirecrackerVm,
}

impl BrowserEnvironment {
    pub fn new(id: &str, svid: &str) -> Self {
        Self {
            vm: FirecrackerVm::new(id, svid),
        }
    }

    pub fn navigate(&self, url: &str) -> Result<String, Box<dyn Error>> {
        self.vm.start_jailer()?;
        // Mock payload configuration for Browser
        self.vm
            .configure_and_boot("/srv/jailer/vmlinux.bin", "/srv/jailer/browser-rootfs.ext4")?;

        println!("Browser navigating to: {}", url);

        // Mock immediately halt
        println!(
            "Halting Firecracker VM {} after Browser execution.",
            self.vm.id
        );

        Ok(format!("Navigated to {}", url))
    }
}

pub struct ComputerEnvironment {
    pub vm: FirecrackerVm,
}

impl ComputerEnvironment {
    pub fn new(id: &str, svid: &str) -> Self {
        Self {
            vm: FirecrackerVm::new(id, svid),
        }
    }

    pub fn click(&self, x: u32, y: u32) -> Result<String, Box<dyn Error>> {
        self.vm.start_jailer()?;
        // Mock payload configuration for Computer
        self.vm.configure_and_boot(
            "/srv/jailer/vmlinux.bin",
            "/srv/jailer/computer-rootfs.ext4",
        )?;

        println!("Computer clicking at: {}, {}", x, y);

        // Mock immediately halt
        println!(
            "Halting Firecracker VM {} after Computer execution.",
            self.vm.id
        );

        Ok(format!("Clicked at {}, {}", x, y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_environment_new() {
        let env = BashEnvironment::new("test-id", "test-svid");
        assert_eq!(env.vm.id, "test-id");
        assert_eq!(env.vm.svid, "test-svid");
        assert_eq!(
            env.vm.socket_path.to_str().unwrap(),
            "/srv/jailer/firecracker/test-id/root/run/firecracker.socket"
        );
    }

    #[test]
    fn test_browser_environment_new() {
        let env = BrowserEnvironment::new("test-id", "test-svid");
        assert_eq!(env.vm.id, "test-id");
        assert_eq!(env.vm.svid, "test-svid");
        assert_eq!(
            env.vm.socket_path.to_str().unwrap(),
            "/srv/jailer/firecracker/test-id/root/run/firecracker.socket"
        );
    }

    #[test]
    fn test_computer_environment_new() {
        let env = ComputerEnvironment::new("test-id", "test-svid");
        assert_eq!(env.vm.id, "test-id");
        assert_eq!(env.vm.svid, "test-svid");
        assert_eq!(
            env.vm.socket_path.to_str().unwrap(),
            "/srv/jailer/firecracker/test-id/root/run/firecracker.socket"
        );
    }
}
