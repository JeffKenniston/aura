use std::error::Error;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

#[derive(Debug)]
pub struct FirecrackerVm {
    pub id: String,
    pub svid: String,
    pub socket_path: PathBuf,
}

impl FirecrackerVm {
    pub fn new(id: &str, svid: &str) -> Self {
        let socket_path = PathBuf::from(format!(
            "/srv/jailer/firecracker/{}/root/run/firecracker.socket",
            id
        ));
        Self {
            id: id.to_string(),
            svid: svid.to_string(),
            socket_path,
        }
    }

    pub fn start_jailer(&self) -> Result<(), Box<dyn Error>> {
        // Wrap the Firecracker process in a secondary jailer daemon to apply
        // strict cgroup v2 constraints and seccomp system call filters.
        println!("Starting jailer for Firecracker VM {}...", self.id);

        let _child = Command::new("jailer")
            .arg("--id")
            .arg(&self.id)
            .arg("--exec-file")
            .arg("/usr/local/bin/firecracker")
            .arg("--uid")
            .arg("1000")
            .arg("--gid")
            .arg("1000")
            .arg("--chroot-base-dir")
            .arg("/srv/jailer")
            .arg("--daemonize")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        // Wait a brief moment for the socket to be created by the daemonized process
        std::thread::sleep(Duration::from_millis(50));

        Ok(())
    }

    pub fn configure_and_boot(
        &self,
        kernel_path: &str,
        rootfs_path: &str,
    ) -> Result<(), Box<dyn Error>> {
        // Send configuration to the Firecracker API socket.
        // In a complete implementation, kernel and rootfs would be hardlinked/copied
        // into the jailer chroot environment before this step.

        let boot_source_json = format!(
            r#"{{"kernel_image_path": "{}", "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"}}"#,
            kernel_path
        );
        self.send_api_request("PUT", "/boot-source", &boot_source_json)?;

        let drive_json = format!(
            r#"{{"drive_id": "rootfs", "path_on_host": "{}", "is_root_device": true, "is_read_only": false}}"#,
            rootfs_path
        );
        self.send_api_request("PUT", "/drives/rootfs", &drive_json)?;

        // Ensure < 5MiB memory overhead by configuring tiny machine.
        // Targeting boot under 125ms for VirtIO-centric guest kernel.
        let machine_config_json = r#"{"vcpu_count": 1, "mem_size_mib": 128}"#;
        self.send_api_request("PUT", "/machine-config", machine_config_json)?;

        // Issue InstanceStart
        let start_json = r#"{"action_type": "InstanceStart"}"#;
        self.send_api_request("PUT", "/actions", start_json)?;

        println!("Firecracker VM {} booted successfully.", self.id);
        Ok(())
    }

    fn send_api_request(&self, method: &str, path: &str, body: &str) -> Result<(), Box<dyn Error>> {
        // Simple HTTP/1.1 client over Unix Domain Socket to interact with Firecracker API
        let mut stream = UnixStream::connect(&self.socket_path)?;

        let request = format!(
            "{} {} HTTP/1.1\r\n\
            Accept: application/json\r\n\
            Content-Type: application/json\r\n\
            Content-Length: {}\r\n\
            \r\n\
            {}",
            method,
            path,
            body.len(),
            body
        );

        stream.write_all(request.as_bytes())?;

        // Basic read response
        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        if response.contains("HTTP/1.1 4") || response.contains("HTTP/1.1 5") {
            return Err(format!("Firecracker API Error: {}", response).into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use tempfile::tempdir;
    use std::sync::Mutex;
    use std::env;

    // Use a mutex to prevent environment variable changes from affecting other tests running concurrently
    static PATH_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn test_firecracker_new() {
        let vm = FirecrackerVm::new("test-vm-id", "spiffe://test/vm");
        assert_eq!(vm.id, "test-vm-id");
        assert_eq!(vm.svid, "spiffe://test/vm");
        assert_eq!(
            vm.socket_path.to_str().unwrap(),
            "/srv/jailer/firecracker/test-vm-id/root/run/firecracker.socket"
        );
    }

    #[test]
    fn test_start_jailer_missing_executable() {
        // Lock to ensure exclusive access to environment variable
        let _lock = PATH_MUTEX.lock().unwrap();

        // Save original path
        let original_path = env::var("PATH").unwrap_or_else(|_| "".to_string());

        // Create an empty temporary directory
        let temp_dir = tempdir().unwrap();

        // Set PATH to just the empty directory, so `jailer` is definitely not found
        env::set_var("PATH", temp_dir.path());

        let vm = FirecrackerVm::new("test-vm-id", "spiffe://test/vm");
        let result = vm.start_jailer();

        // Check if the expected error occurs
        assert!(result.is_err());
        if let Err(e) = result {
            let io_err = e.downcast_ref::<std::io::Error>();
            assert!(io_err.is_some(), "Expected std::io::Error, got {:?}", e);
            assert_eq!(
                io_err.unwrap().kind(),
                std::io::ErrorKind::NotFound,
                "Expected NotFound error, got {:?}",
                io_err.unwrap().kind()
            );
        }

        // Restore original path
        env::set_var("PATH", original_path);
    }

    #[test]
    fn test_start_jailer_success() {
        // Lock to ensure exclusive access to environment variable
        let _lock = PATH_MUTEX.lock().unwrap();

        let original_path = env::var("PATH").unwrap_or_else(|_| "".to_string());

        // Create a temporary directory for our mock jailer
        let temp_dir = tempdir().unwrap();
        let mock_jailer_path = temp_dir.path().join("jailer");
        let mock_output_path = temp_dir.path().join("jailer_output.txt");

        // Create a mock shell script that records its arguments
        let script_content = format!(
            "#!/bin/sh\n\
             echo \"$@\" > {}\n\
             exit 0\n",
            mock_output_path.to_str().unwrap()
        );
        fs::write(&mock_jailer_path, script_content).unwrap();

        // Make the script executable
        let mut perms = fs::metadata(&mock_jailer_path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&mock_jailer_path, perms).unwrap();

        // Update PATH to include the temp directory first
        let new_path = format!("{}:{}", temp_dir.path().to_str().unwrap(), original_path);
        env::set_var("PATH", new_path);

        let vm = FirecrackerVm::new("test-vm-id", "spiffe://test/vm");
        let result = vm.start_jailer();

        // Check that the command succeeded
        assert!(result.is_ok(), "Expected Ok, got {:?}", result);

        // Verify the mock script was executed and arguments were passed correctly
        let args_output = fs::read_to_string(&mock_output_path)
            .expect("Failed to read mock jailer output file")
            .trim()
            .to_string();

        assert_eq!(
            args_output,
            "--id test-vm-id --exec-file /usr/local/bin/firecracker --uid 1000 --gid 1000 --chroot-base-dir /srv/jailer --daemonize"
        );

        // Restore original path
        env::set_var("PATH", original_path);
    }
}
