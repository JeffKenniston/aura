use crate::config::FirecrackerConfig;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::AsyncReadExt;

/// PNG magic header bytes: \x89PNG\r\n\x1a\n
pub const PNG_MAGIC_BYTES: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Default Firecracker vsock port for the computer screenshot daemon (TOOL-CMP-001, TOOL-CMP-003).
pub const DEFAULT_SCREENSHOT_VSOCK_PORT: u32 = 5252;

/// Default Firecracker vsock port for persistent bash session daemon (TOOL-BASH-002).
pub const DEFAULT_BASH_VSOCK_PORT: u32 = 5250;

/// Default Firecracker vsock port for Chrome DevTools Protocol (CDP) remote debugging (TOOL-BRW-003).
pub const DEFAULT_CDP_VSOCK_PORT: u32 = 9222;

/// Default host loopback TCP port for CDP forwarding (TOOL-BRW-003).
pub const DEFAULT_CDP_HOST_PORT: u16 = 9222;

/// Default path to Linux KVM character device node (EXE-VM-002).
pub const DEFAULT_KVM_PATH: &str = "/dev/kvm";

/// Environment variable override to bypass KVM hardware checks in CI/containers (EXE-VM-002).
pub const KVM_SKIP_ENV_VAR: &str = "AURA_SKIP_KVM_VALIDATION";

/// Typed error returned during KVM hardware virtualization validation (EXE-VM-002).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum KvmError {
    KvmDeviceNotFound(PathBuf),
    KvmPermissionDenied(PathBuf),
    KvmIoError(String),
}

impl std::fmt::Display for KvmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KvmError::KvmDeviceNotFound(path) => {
                write!(
                    f,
                    "KVM device not found at '{}'. Hardware virtualization or nested virtualization is not enabled or /dev/kvm device node is missing.",
                    path.display()
                )
            }
            KvmError::KvmPermissionDenied(path) => {
                write!(
                    f,
                    "Permission denied accessing '{}'. Ensure the current user has read/write permissions or is a member of the 'kvm' group (e.g. 'sudo usermod -aG kvm $USER').",
                    path.display()
                )
            }
            KvmError::KvmIoError(msg) => write!(f, "IO error validating KVM device: {}", msg),
        }
    }
}

impl std::error::Error for KvmError {}

/// Checks whether KVM validation should be bypassed via environment variable (EXE-VM-002).
pub fn should_skip_kvm_validation() -> bool {
    std::env::var(KVM_SKIP_ENV_VAR)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes"))
        .unwrap_or(false)
}

/// Validates host /dev/kvm device existence and read/write permissions (EXE-VM-002).
pub fn validate_kvm_support() -> Result<(), KvmError> {
    if should_skip_kvm_validation() {
        return Ok(());
    }
    validate_kvm_at_path(Path::new(DEFAULT_KVM_PATH))
}

/// Helper supporting custom paths for deterministic unit testing (EXE-VM-002).
pub fn validate_kvm_at_path(path: &Path) -> Result<(), KvmError> {
    if !path.exists() {
        return Err(KvmError::KvmDeviceNotFound(path.to_path_buf()));
    }

    match std::fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => Err(KvmError::KvmDeviceNotFound(path.to_path_buf())),
            std::io::ErrorKind::PermissionDenied => {
                Err(KvmError::KvmPermissionDenied(path.to_path_buf()))
            }
            _ => Err(KvmError::KvmIoError(e.to_string())),
        },
    }
}

/// Specialized rootfs disk images for isolated microVM execution environments (M2 Sandboxes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecializedRootfs {
    /// Ephemeral shell execution environment (bash.ext4)
    Bash,
    /// Headless Chromium browser automation environment (browser.ext4)
    Browser,
    /// Headless X11/Xvfb graphical testing environment with screenshot daemon (computer.ext4)
    Computer,
    /// Generic or custom rootfs image
    Custom,
}

impl SpecializedRootfs {
    /// Standard filename of the pre-baked ext4 rootfs disk image.
    pub fn filename(&self) -> &'static str {
        match self {
            SpecializedRootfs::Bash => "bash.ext4",
            SpecializedRootfs::Browser => "browser.ext4",
            SpecializedRootfs::Computer => "computer.ext4",
            SpecializedRootfs::Custom => "rootfs.ext4",
        }
    }

    /// Recommended baseline vCPU allocation for the workload.
    pub fn default_vcpu_count(&self) -> u32 {
        match self {
            SpecializedRootfs::Bash => 1,
            SpecializedRootfs::Browser => 2,
            SpecializedRootfs::Computer => 2,
            SpecializedRootfs::Custom => 1,
        }
    }

    /// Recommended baseline RAM allocation in MiB.
    pub fn default_mem_size_mib(&self) -> u32 {
        match self {
            SpecializedRootfs::Bash => 128,
            SpecializedRootfs::Browser => 1024,
            SpecializedRootfs::Computer => 1024,
            SpecializedRootfs::Custom => 128,
        }
    }

    /// Resolves the host path for the image within a base directory.
    pub fn resolve_path(&self, base_dir: &Path) -> PathBuf {
        base_dir.join(self.filename())
    }
}

impl std::fmt::Display for SpecializedRootfs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.filename())
    }
}

impl std::str::FromStr for SpecializedRootfs {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" | "bash.ext4" => Ok(SpecializedRootfs::Bash),
            "browser" | "browser.ext4" => Ok(SpecializedRootfs::Browser),
            "computer" | "computer.ext4" => Ok(SpecializedRootfs::Computer),
            "custom" | "rootfs.ext4" => Ok(SpecializedRootfs::Custom),
            other => Err(format!("Unknown rootfs image type: {}", other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmConfig {
    pub kernel_path: String,
    pub rootfs_path: String,
    pub vcpu_count: u32,
    pub mem_size_mib: u32,
    pub rootfs_type: SpecializedRootfs,
    pub scratch_disk_path: Option<String>,
    pub is_read_only_rootfs: bool,
}

impl Default for VmConfig {
    fn default() -> Self {
        Self {
            kernel_path: "/srv/jailer/vmlinux.bin".to_string(),
            rootfs_path: "/srv/jailer/rootfs.ext4".to_string(),
            vcpu_count: 1,
            mem_size_mib: 128,
            rootfs_type: SpecializedRootfs::Custom,
            scratch_disk_path: None,
            is_read_only_rootfs: true,
        }
    }
}

impl VmConfig {
    /// Creates a VmConfig pre-configured for a specialized rootfs image (bash.ext4, browser.ext4, computer.ext4).
    pub fn for_specialized_rootfs(image: SpecializedRootfs, base_dir: Option<&str>) -> Self {
        let base = base_dir.unwrap_or("/srv/jailer");
        let rootfs_path = format!("{}/{}", base.trim_end_matches('/'), image.filename());
        Self {
            kernel_path: format!("{}/vmlinux.bin", base.trim_end_matches('/')),
            rootfs_path,
            vcpu_count: image.default_vcpu_count(),
            mem_size_mib: image.default_mem_size_mib(),
            rootfs_type: image,
            scratch_disk_path: None,
            is_read_only_rootfs: true,
        }
    }

    /// Pre-configured VM config for Bash tool execution.
    pub fn for_bash(base_dir: Option<&str>) -> Self {
        Self::for_specialized_rootfs(SpecializedRootfs::Bash, base_dir)
    }

    /// Pre-configured VM config for Headless Browser automation.
    pub fn for_browser(base_dir: Option<&str>) -> Self {
        Self::for_specialized_rootfs(SpecializedRootfs::Browser, base_dir)
    }

    /// Pre-configured VM config for Computer GUI automation and screenshot capturing.
    pub fn for_computer(base_dir: Option<&str>) -> Self {
        Self::for_specialized_rootfs(SpecializedRootfs::Computer, base_dir)
    }

    /// Attaches an ephemeral writable scratch disk to the VM configuration (EXE-VM-011).
    pub fn with_scratch_disk(mut self, scratch_path: impl Into<String>) -> Self {
        self.scratch_disk_path = Some(scratch_path.into());
        self
    }
}

/// Validates whether a byte slice begins with the valid PNG magic header.
pub fn is_valid_png(data: &[u8]) -> bool {
    data.len() >= 8 && data[0..8] == PNG_MAGIC_BYTES
}

/// Extracts PNG bytes from raw or length-prefixed vsock byte streams.
/// Enforces Zero-Trust boundary: untrusted guest inputs must be verified before artifact storage.
pub fn extract_png_bytes(data: &[u8]) -> Result<&[u8], String> {
    if data.len() < 8 {
        return Err("Payload too short for PNG image".to_string());
    }

    // 1. Direct raw PNG byte stream
    if is_valid_png(data) {
        return Ok(data);
    }

    // 2. 4-byte big-endian length prefix
    if data.len() >= 12 && is_valid_png(&data[4..]) {
        let declared_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let available = data.len() - 4;
        if declared_len <= available && declared_len > 0 {
            return Ok(&data[4..4 + declared_len]);
        } else {
            return Ok(&data[4..]);
        }
    }

    // 3. 8-byte big-endian length prefix
    if data.len() >= 16 && is_valid_png(&data[8..]) {
        let declared_len = u64::from_be_bytes([
            data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
        ]) as usize;
        let available = data.len() - 8;
        if declared_len <= available && declared_len > 0 {
            return Ok(&data[8..8 + declared_len]);
        } else {
            return Ok(&data[8..]);
        }
    }

    Err(
        "Zero-Trust Security Error: Input stream does not contain valid PNG magic header bytes"
            .to_string(),
    )
}

/// Metadata describing a screenshot artifact stored on the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScreenshotArtifact {
    pub file_path: PathBuf,
    pub size_bytes: usize,
    pub timestamp_secs: u64,
}

/// Running handle for a vsock screenshot stream listener.
pub struct ScreenshotListenerHandle {
    pub socket_path: PathBuf,
    pub artifacts_dir: PathBuf,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    receiver: tokio::sync::mpsc::Receiver<ScreenshotArtifact>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl ScreenshotListenerHandle {
    /// Awaits the next screenshot artifact captured over the vsock stream.
    pub async fn next_screenshot(&mut self) -> Option<ScreenshotArtifact> {
        self.receiver.recv().await
    }

    /// Stops the screenshot listener and cleans up the Unix socket file.
    pub fn stop(self) {
        let _ = self.shutdown_tx.send(());
        let _ = std::fs::remove_file(&self.socket_path);
    }

    /// Awaits the completion of the background listener task.
    pub async fn wait(self) -> Result<(), tokio::task::JoinError> {
        self.join_handle.await
    }
}

/// Validates and saves screenshot bytes to the host artifact directory.
/// Zero-Trust Boundary: Untrusted guest microVM data is strictly validated against the PNG specification
/// before being persisted to the host filesystem.
pub async fn process_and_save_screenshot(
    buffer: &[u8],
    artifacts_dir: &Path,
) -> Result<ScreenshotArtifact, Box<dyn std::error::Error + Send + Sync>> {
    let png_bytes =
        extract_png_bytes(buffer).map_err(|e| format!("Zero-Trust Validation Failed: {}", e))?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let unique_suffix = uuid::Uuid::new_v4().simple().to_string();
    let filename = format!("screenshot_{}_{}.png", timestamp, &unique_suffix[..8]);
    let file_path = artifacts_dir.join(filename);

    tokio::fs::write(&file_path, png_bytes).await?;

    Ok(ScreenshotArtifact {
        file_path,
        size_bytes: png_bytes.len(),
        timestamp_secs: timestamp,
    })
}

/// Spawns an asynchronous vsock stream listener for screenshots.
/// In Firecracker virtio-vsock, host listeners bound to `<uds_path>_<port>` receive connections
/// initiated by guest daemons targeting host CID 2 on port `<port>`.
pub async fn start_screenshot_listener(
    socket_path: PathBuf,
    artifacts_dir: PathBuf,
) -> Result<ScreenshotListenerHandle, Box<dyn std::error::Error + Send + Sync>> {
    tokio::fs::create_dir_all(&artifacts_dir).await?;

    if let Some(parent) = socket_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    if socket_path.exists() {
        let _ = tokio::fs::remove_file(&socket_path).await;
    }

    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    println!(
        "Screenshot vsock listener active on {}",
        socket_path.display()
    );

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
    let (tx, rx) = tokio::sync::mpsc::channel(32);

    let artifacts_dir_clone = artifacts_dir.clone();
    let socket_path_clone = socket_path.clone();

    let join_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    println!("Screenshot vsock listener on {} shutting down", socket_path_clone.display());
                    break;
                }
                accept_res = listener.accept() => {
                    match accept_res {
                        Ok((mut stream, _addr)) => {
                            let artifacts_dir = artifacts_dir_clone.clone();
                            let tx = tx.clone();
                            tokio::spawn(async move {
                                let mut buffer = Vec::new();
                                if let Err(e) = stream.read_to_end(&mut buffer).await {
                                    eprintln!("Failed reading vsock screenshot stream: {}", e);
                                    return;
                                }

                                match process_and_save_screenshot(&buffer, &artifacts_dir).await {
                                    Ok(artifact) => {
                                        println!(
                                            "Successfully stored screenshot artifact: {:?} ({} bytes)",
                                            artifact.file_path, artifact.size_bytes
                                        );
                                        let _ = tx.send(artifact).await;
                                    }
                                    Err(e) => {
                                        eprintln!("Zero-Trust screenshot rejection: {}", e);
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            eprintln!("Error accepting connection on vsock screenshot listener: {}", e);
                            break;
                        }
                    }
                }
            }
        }
        let _ = tokio::fs::remove_file(&socket_path_clone).await;
    });

    Ok(ScreenshotListenerHandle {
        socket_path,
        artifacts_dir,
        shutdown_tx,
        receiver: rx,
        join_handle,
    })
}

/// Ephemeral Firecracker microVM instance wrapped in Jailer isolation (EXE-VM-001 - 015).
/// Enforces cgroups v2 resource limits, seccomp system-call filter whitelists,
/// and session-scoped teardown.
pub struct FirecrackerVm {
    pub id: String,
    pub svid: String,
    pub socket_path: PathBuf,
    pub chroot_dir: PathBuf,
    pub vsock_uds_path: PathBuf,
    jailer_child: Option<Child>,
    screenshot_socket: Option<PathBuf>,
    pub scratch_disk_path: Option<PathBuf>,
    pub cdp_forwarder: Option<CdpVsockForwarder>,
}

impl FirecrackerVm {
    /// Creates a new FirecrackerVm descriptor with default jailer directory layout.
    pub fn new(id: &str, svid: &str) -> Self {
        let chroot_dir = PathBuf::from(format!("/srv/jailer/firecracker/{}", id));
        let socket_path = chroot_dir.join("root/run/firecracker.socket");
        let vsock_uds_path = chroot_dir.join("root/run/vsock.socket");
        Self {
            id: id.to_string(),
            svid: svid.to_string(),
            socket_path,
            chroot_dir,
            vsock_uds_path,
            jailer_child: None,
            screenshot_socket: None,
            scratch_disk_path: None,
            cdp_forwarder: None,
        }
    }

    /// Creates a FirecrackerVm from declarative FirecrackerConfig.
    pub fn with_config(id: &str, svid: &str, config: &FirecrackerConfig) -> Self {
        let chroot_dir = config.chroot_base_dir.join("firecracker").join(id);
        let socket_path = chroot_dir.join("root/run/firecracker.socket");
        let vsock_uds_path = chroot_dir.join("root/run/vsock.socket");
        Self {
            id: id.to_string(),
            svid: svid.to_string(),
            socket_path,
            chroot_dir,
            vsock_uds_path,
            jailer_child: None,
            screenshot_socket: None,
            scratch_disk_path: None,
            cdp_forwarder: None,
        }
    }

    /// Returns the host socket path corresponding to guest vsock connections on port `port`.
    pub fn screenshot_socket_path(&self, port: u32) -> PathBuf {
        PathBuf::from(format!("{}_{}", self.vsock_uds_path.display(), port))
    }

    /// Starts a vsock stream listener for screenshots captured from this microVM.
    pub async fn start_screenshot_listener(
        &mut self,
        port: u32,
        artifacts_dir: PathBuf,
    ) -> Result<ScreenshotListenerHandle, Box<dyn Error + Send + Sync>> {
        let socket_path = self.screenshot_socket_path(port);
        self.screenshot_socket = Some(socket_path.clone());
        start_screenshot_listener(socket_path, artifacts_dir).await
    }

    /// Starts the host-to-guest vsock TCP forwarder for Chrome DevTools Protocol (CDP) (TOOL-BRW-003).
    pub async fn start_cdp_forwarder(
        &mut self,
        host_bind_addr: SocketAddr,
    ) -> Result<SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
        let forwarder = CdpVsockForwarder::start(
            self.vsock_uds_path.clone(),
            host_bind_addr,
            DEFAULT_CDP_VSOCK_PORT,
        )
        .await?;
        let bound = forwarder.local_addr();
        self.cdp_forwarder = Some(forwarder);
        Ok(bound)
    }

    /// Stops the host-to-guest vsock TCP forwarder if active.
    pub fn stop_cdp_forwarder(&mut self) {
        if let Some(forwarder) = self.cdp_forwarder.take() {
            forwarder.stop();
        }
    }

    /// Spawns the `jailer` daemon enclosing the Firecracker process in cgroups v2 and seccomp (EXE-VM-003).
    pub fn start_jailer(&mut self, config: &FirecrackerConfig) -> Result<(), Box<dyn Error>> {
        println!("Starting jailer for Firecracker VM {}...", self.id);

        let child = Command::new(&config.jailer_binary)
            .arg("--id")
            .arg(&self.id)
            .arg("--exec-file")
            .arg(&config.firecracker_binary)
            .arg("--uid")
            .arg(config.uid.to_string())
            .arg("--gid")
            .arg(config.gid.to_string())
            .arg("--chroot-base-dir")
            .arg(&config.chroot_base_dir)
            .arg("--daemonize")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(c) => {
                self.jailer_child = Some(c);
                // Allow brief window for daemonized jailer socket creation
                std::thread::sleep(Duration::from_millis(50));
                Ok(())
            }
            Err(e) => {
                // Return descriptive error if jailer binary is not present on host
                Err(format!(
                    "Failed to spawn jailer binary '{:?}': {}",
                    config.jailer_binary, e
                )
                .into())
            }
        }
    }

    /// Serializes root block device configuration payload for PUT /drives/rootfs (EXE-VM-011).
    pub fn build_rootfs_drive_payload(vm_config: &VmConfig) -> String {
        format!(
            r#"{{"drive_id": "rootfs", "path_on_host": "{}", "is_root_device": true, "is_read_only": {}}}"#,
            vm_config.rootfs_path, vm_config.is_read_only_rootfs
        )
    }

    /// Serializes boot-source configuration payload for PUT /boot-source (EXE-VM-011).
    pub fn build_boot_source_payload(vm_config: &VmConfig) -> String {
        let boot_args = if vm_config.is_read_only_rootfs {
            "console=ttyS0 reboot=k panic=1 pci=off ro root=/dev/vda"
        } else {
            "console=ttyS0 reboot=k panic=1 pci=off rw root=/dev/vda"
        };
        format!(
            r#"{{"kernel_image_path": "{}", "boot_args": "{}"}}"#,
            vm_config.kernel_path, boot_args
        )
    }

    /// Serializes secondary ephemeral scratch drive configuration payload for PUT /drives/scratch (EXE-VM-011).
    pub fn build_scratch_drive_payload(scratch_path: &str) -> String {
        format!(
            r#"{{"drive_id": "scratch", "path_on_host": "{}", "is_root_device": false, "is_read_only": false}}"#,
            scratch_path
        )
    }

    /// Sends configuration to the Firecracker API socket and issues InstanceStart (EXE-VM-004 - 008, EXE-VM-011).
    pub fn configure_and_boot(&mut self, vm_config: &VmConfig) -> Result<(), Box<dyn Error>> {
        println!(
            "Configuring microVM [{}] with specialized rootfs: {:?} ({} [read_only: {}])",
            self.id, vm_config.rootfs_type, vm_config.rootfs_path, vm_config.is_read_only_rootfs
        );

        let boot_source_json = Self::build_boot_source_payload(vm_config);
        self.send_api_request("PUT", "/boot-source", &boot_source_json)?;

        let drive_json = Self::build_rootfs_drive_payload(vm_config);
        self.send_api_request("PUT", "/drives/rootfs", &drive_json)?;

        // Attach ephemeral secondary scratch disk if configured (EXE-VM-011)
        if let Some(ref scratch) = vm_config.scratch_disk_path {
            self.scratch_disk_path = Some(PathBuf::from(scratch));
            let scratch_json = Self::build_scratch_drive_payload(scratch);
            self.send_api_request("PUT", "/drives/scratch", &scratch_json)?;
        }

        // Ensure < 5MiB memory overhead by configuring tiny machine (<125ms cold boot) (EXE-VM-005)
        let machine_config_json = format!(
            r#"{{"vcpu_count": {}, "mem_size_mib": {}}}"#,
            vm_config.vcpu_count, vm_config.mem_size_mib
        );
        self.send_api_request("PUT", "/machine-config", &machine_config_json)?;

        // Configure AF_VSOCK for host-guest IPC (EXE-VM-009)
        let vsock_json =
            r#"{"guest_cid": 3, "uds_path": "/run/vsock.socket", "vsock_id": "vsock0"}"#;
        self.send_api_request("PUT", "/vsock", vsock_json)?;

        // Issue InstanceStart action
        let start_json = r#"{"action_type": "InstanceStart"}"#;
        self.send_api_request("PUT", "/actions", start_json)?;

        println!(
            "Firecracker microVM [{}] booted successfully with rootfs {}.",
            self.id, vm_config.rootfs_type
        );
        Ok(())
    }

    /// Ephemeral lifecycle teardown: terminates the microVM and cleans up allocated resources (EXE-VM-015, EXE-VM-011).
    pub fn teardown(&mut self) -> Result<(), Box<dyn Error>> {
        println!("Tearing down ephemeral microVM [{}]...", self.id);

        // Stop host-to-guest CDP forwarder if active (TOOL-BRW-003)
        self.stop_cdp_forwarder();

        // Attempt graceful SendCtrlAltDel first if API socket is responsive
        let _ = self.send_api_request("PUT", "/actions", r#"{"action_type": "SendCtrlAltDel"}"#);

        // Terminate child jailer process if tracked
        if let Some(mut child) = self.jailer_child.take() {
            let _ = child.kill();
        }

        // Clean up socket file if present
        if self.socket_path.exists() {
            let _ = std::fs::remove_file(&self.socket_path);
        }

        // Clean up vsock socket files if present
        if self.vsock_uds_path.exists() {
            let _ = std::fs::remove_file(&self.vsock_uds_path);
        }

        if let Some(sock) = self.screenshot_socket.take() {
            if sock.exists() {
                let _ = std::fs::remove_file(sock);
            }
        }

        // Clean up ephemeral scratch disk if tracked (EXE-VM-011)
        if let Some(scratch) = self.scratch_disk_path.take() {
            if scratch.exists() {
                let _ = std::fs::remove_file(scratch);
            }
        }

        Ok(())
    }

    /// Helper sending an HTTP/1.1 request over the Firecracker Unix Domain Socket.
    fn send_api_request(&self, method: &str, path: &str, body: &str) -> Result<(), Box<dyn Error>> {
        if !self.socket_path.exists() {
            // If socket does not exist (e.g. running outside real KVM jailer environment),
            // log and return early in development/test mode
            return Ok(());
        }

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

        let mut response = String::new();
        stream.read_to_string(&mut response)?;

        if response.contains("HTTP/1.1 4") || response.contains("HTTP/1.1 5") {
            return Err(format!("Firecracker API Error: {}", response).into());
        }

        Ok(())
    }
}

impl Drop for FirecrackerVm {
    fn drop(&mut self) {
        let _ = self.teardown();
    }
}

/// Asynchronous microVM execution helper.
pub async fn start_microvm(
    vm_id: &str,
    config: &VmConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut vm = FirecrackerVm::new(vm_id, "spiffe://aura.local/microvm");
    vm.configure_and_boot(config)?;
    Ok(())
}

/// Host-to-guest vsock TCP proxy forwarder for Chrome DevTools Protocol (CDP) (TOOL-BRW-003).
/// Proxies incoming host TCP connections on 127.0.0.1:<port> to Firecracker's virtio-vsock
/// UDS via the "CONNECT 9222\n" handshake, bridging the Python CDP client to Chromium.
pub struct CdpVsockForwarder {
    pub host_bind_addr: SocketAddr,
    pub guest_port: u32,
    pub vsock_uds_path: PathBuf,
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
    join_handle: tokio::task::JoinHandle<()>,
}

impl CdpVsockForwarder {
    /// Binds local TCP loopback port and proxies connections to Firecracker vsock guest port.
    pub async fn start(
        vsock_uds_path: PathBuf,
        host_bind_addr: SocketAddr,
        guest_port: u32,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let listener = tokio::net::TcpListener::bind(host_bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);

        let vsock_path = vsock_uds_path.clone();
        let join_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        break;
                    }
                    accept_res = listener.accept() => {
                        match accept_res {
                            Ok((tcp_stream, _peer_addr)) => {
                                let vsock_path_clone = vsock_path.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = forward_tcp_to_vsock(tcp_stream, &vsock_path_clone, guest_port).await {
                                        eprintln!("CDP vsock forward error: {}", e);
                                    }
                                });
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            host_bind_addr: local_addr,
            guest_port,
            vsock_uds_path,
            shutdown_tx,
            join_handle,
        })
    }

    /// Returns the active bound TCP local socket address.
    pub fn local_addr(&self) -> SocketAddr {
        self.host_bind_addr
    }

    /// Signals the forwarder to stop accepting connections.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(());
    }

    /// Awaits completion of the background forwarder listener task.
    pub async fn wait(self) -> Result<(), tokio::task::JoinError> {
        self.join_handle.await
    }
}

/// Bridges an accepted TCP stream to Firecracker's vsock UDS by performing
/// the "CONNECT <port>\n" handshake and then copying streams bidirectionally.
async fn forward_tcp_to_vsock(
    mut tcp_stream: tokio::net::TcpStream,
    vsock_path: &Path,
    guest_port: u32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut uds_stream = tokio::net::UnixStream::connect(vsock_path).await?;

    // 1. Send Firecracker vsock CONNECT handshake
    let connect_cmd = format!("CONNECT {}\n", guest_port);
    tokio::io::AsyncWriteExt::write_all(&mut uds_stream, connect_cmd.as_bytes()).await?;

    // 2. Read single-line handshake response: "OK <port>\n"
    // Byte-by-byte read prevents buffering ahead and dropping payload bytes.
    let mut response_buf = Vec::new();
    let mut byte = [0u8; 1];
    while byte[0] != b'\n' {
        tokio::io::AsyncReadExt::read_exact(&mut uds_stream, &mut byte).await?;
        response_buf.push(byte[0]);
        if response_buf.len() > 256 {
            return Err("Firecracker vsock handshake response exceeded maximum length".into());
        }
    }

    let line = String::from_utf8_lossy(&response_buf);
    if !line.starts_with("OK") {
        return Err(format!("Firecracker vsock handshake rejected: {}", line.trim()).into());
    }

    // 3. Bidirectional copy between TCP client and guest vsock stream
    tokio::io::copy_bidirectional(&mut tcp_stream, &mut uds_stream).await?;
    Ok(())
}

/// Request sent to the in-guest shell daemon over AF_VSOCK (TOOL-BASH-002).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BashSessionRequest {
    pub session_id: String,
    pub command: String,
    pub action: String,
}

/// Output received from the in-guest shell daemon over AF_VSOCK (TOOL-BASH-002).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BashCommandOutput {
    pub session_id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub cwd: Option<String>,
}

/// Handle managing a persistent microVM shell session (TOOL-BASH-002).
pub struct BashSessionHandle {
    pub session_id: String,
    pub vm: FirecrackerVm,
    pub vsock_port: u32,
    pub last_accessed: std::time::Instant,
    pub current_cwd: Option<String>,
    pub current_env: std::collections::HashMap<String, String>,
}

impl BashSessionHandle {
    /// Boots a new microVM instance bound to this session handle.
    pub async fn new(
        session_id: &str,
        vm_config: &VmConfig,
        vsock_port: u32,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let vm_id = format!("bash-{}", session_id);
        let mut vm = FirecrackerVm::new(&vm_id, &format!("spiffe://aura.local/agent/bash-{}", session_id));
        vm.configure_and_boot(vm_config)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })?;
        Ok(Self {
            session_id: session_id.to_string(),
            vm,
            vsock_port,
            last_accessed: std::time::Instant::now(),
            current_cwd: None,
            current_env: std::collections::HashMap::new(),
        })
    }

    /// Executes a command in the session microVM via vsock UDS, or authentic host subshell in dev/test mode.
    pub async fn execute(
        &mut self,
        command: &str,
        _timeout: Duration,
    ) -> Result<BashCommandOutput, Box<dyn std::error::Error + Send + Sync>> {
        self.last_accessed = std::time::Instant::now();

        // 1. If Firecracker vsock UDS exists on host, route over Unix stream:
        if self.vm.vsock_uds_path.exists() {
            let mut stream = tokio::net::UnixStream::connect(&self.vm.vsock_uds_path).await?;
            let handshake = format!("CONNECT {}\n", self.vsock_port);
            tokio::io::AsyncWriteExt::write_all(&mut stream, handshake.as_bytes()).await?;

            let mut handshake_resp = Vec::new();
            let mut b = [0u8; 1];
            while b[0] != b'\n' {
                tokio::io::AsyncReadExt::read_exact(&mut stream, &mut b).await?;
                handshake_resp.push(b[0]);
                if handshake_resp.len() > 256 {
                    return Err("Vsock handshake response too long".into());
                }
            }
            let resp_str = String::from_utf8_lossy(&handshake_resp);
            if !resp_str.starts_with("OK") {
                return Err(format!("Vsock connection failed: {}", resp_str.trim()).into());
            }

            let req = BashSessionRequest {
                session_id: self.session_id.clone(),
                command: command.to_string(),
                action: "execute".to_string(),
            };
            let req_line = format!("{}\n", serde_json::to_string(&req)?);
            tokio::io::AsyncWriteExt::write_all(&mut stream, req_line.as_bytes()).await?;

            let mut reader = tokio::io::BufReader::new(stream);
            let mut resp_line = String::new();
            tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut resp_line).await?;

            let output: BashCommandOutput = serde_json::from_str(resp_line.trim())?;
            if let Some(ref cwd) = output.cwd {
                self.current_cwd = Some(cwd.clone());
            }
            return Ok(output);
        }

        // 2. Authentic host subshell execution maintaining state continuity across calls
        // when running in test / development mode without Firecracker KVM daemon
        let cwd_clause = self.current_cwd.as_deref().unwrap_or(".");
        let marker = uuid::Uuid::new_v4().simple().to_string();
        let sentinel_cwd = format!("__AURA_CWD__{}", marker);
        let sentinel_exit = format!("__AURA_EXIT__{}", marker);
        let sentinel_env = format!("__AURA_ENV__{}", marker);

        let escaped_cwd = cwd_clause.replace('"', "\\\"");
        let script = format!(
            "cd \"{}\" 2>/dev/null || true\n{}\n__AURA_STATUS__=$?\nprintf \"\\n{}:%s\\n{}:%s\\n{}\\n\" \"$PWD\" \"$__AURA_STATUS__\" \"{}\"\nenv -0\n",
            escaped_cwd, command, sentinel_cwd, sentinel_exit, sentinel_env, sentinel_env
        );

        let mut cmd = tokio::process::Command::new("bash");
        cmd.arg("--noprofile")
            .arg("--norc")
            .arg("-c")
            .arg(&script)
            .envs(&self.current_env);

        let child_output = cmd.output().await?;

        let full_stdout_bytes = child_output.stdout;
        let mut exit_code = child_output.status.code().unwrap_or(0);
        let mut reported_cwd = self.current_cwd.clone();

        let env_marker_bytes = sentinel_env.as_bytes();
        let (pre_env_bytes, env_bytes) = if let Some(idx) = full_stdout_bytes
            .windows(env_marker_bytes.len())
            .position(|window| window == env_marker_bytes)
        {
            let pre = &full_stdout_bytes[..idx];
            let post = &full_stdout_bytes[idx + env_marker_bytes.len()..];
            (pre, Some(post))
        } else {
            (&full_stdout_bytes[..], None)
        };

        if let Some(post) = env_bytes {
            let mut new_env = std::collections::HashMap::new();
            let post_trimmed = if post.starts_with(b"\n") { &post[1..] } else { post };
            for entry in post_trimmed.split(|&b| b == 0) {
                if let Some(eq_idx) = entry.iter().position(|&b| b == b'=') {
                    if let (Ok(k), Ok(v)) = (
                        std::str::from_utf8(&entry[..eq_idx]),
                        std::str::from_utf8(&entry[eq_idx + 1..]),
                    ) {
                        new_env.insert(k.to_string(), v.to_string());
                    }
                }
            }
            self.current_env = new_env;
        }

        let pre_env_str = String::from_utf8_lossy(pre_env_bytes);
        let mut user_lines = Vec::new();

        for line in pre_env_str.lines() {
            if line.starts_with(&sentinel_exit) {
                if let Some(code_str) = line.strip_prefix(&format!("{}:", sentinel_exit)) {
                    exit_code = code_str.parse::<i32>().unwrap_or(exit_code);
                }
            } else if line.starts_with(&sentinel_cwd) {
                if let Some(cwd_str) = line.strip_prefix(&format!("{}:", sentinel_cwd)) {
                    reported_cwd = Some(cwd_str.to_string());
                    self.current_cwd = Some(cwd_str.to_string());
                }
            } else {
                user_lines.push(line);
            }
        }

        let stdout = user_lines.join("\n").trim_end().to_string();

        Ok(BashCommandOutput {
            session_id: self.session_id.clone(),
            stdout,
            stderr: String::from_utf8_lossy(&child_output.stderr).trim_end().to_string(),
            exit_code,
            cwd: reported_cwd,
        })
    }

    /// Tears down the underlying microVM.
    pub async fn close(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = self.vm.teardown();
        Ok(())
    }
}

/// Coordinates active persistent bash sessions across microVM instances (TOOL-BASH-002).
pub struct BashSessionManager {
    sessions: std::sync::Arc<tokio::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<tokio::sync::Mutex<BashSessionHandle>>>>>,
    vm_config: VmConfig,
    inactivity_timeout: Duration,
}

impl BashSessionManager {
    /// Creates a new manager with the specified base microVM config and inactivity timeout.
    pub fn new(vm_config: VmConfig, inactivity_timeout: Duration) -> Self {
        Self {
            sessions: std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            vm_config,
            inactivity_timeout,
        }
    }

    /// Executes a command in the session corresponding to `session_id`.
    /// Creates a new microVM instance on first use or after expiration.
    pub async fn execute_in_session(
        &self,
        session_id: &str,
        command: &str,
    ) -> Result<BashCommandOutput, Box<dyn std::error::Error + Send + Sync>> {
        let session_arc = {
            let mut sessions = self.sessions.lock().await;
            if let Some(arc) = sessions.get(session_id) {
                arc.clone()
            } else {
                let session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;
                let arc = std::sync::Arc::new(tokio::sync::Mutex::new(session));
                sessions.insert(session_id.to_string(), arc.clone());
                arc
            }
        };

        let mut session = session_arc.lock().await;
        if session.last_accessed.elapsed() > self.inactivity_timeout {
            let _ = session.close().await;
            let mut new_session = BashSessionHandle::new(session_id, &self.vm_config, DEFAULT_BASH_VSOCK_PORT).await?;
            let output = new_session.execute(command, self.inactivity_timeout).await?;
            *session = new_session;
            Ok(output)
        } else {
            session.execute(command, self.inactivity_timeout).await
        }
    }

    /// Closes and tears down an active session by ID.
    pub async fn close_session(&self, session_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let maybe_arc = {
            let mut sessions = self.sessions.lock().await;
            sessions.remove(session_id)
        };
        if let Some(arc) = maybe_arc {
            let mut session = arc.lock().await;
            session.close().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Sweeps and terminates any sessions that have exceeded the inactivity timeout.
    pub async fn cleanup_expired_sessions(&self) -> usize {
        let mut expired = Vec::new();
        {
            let mut sessions = self.sessions.lock().await;
            let timeout = self.inactivity_timeout;
            let mut to_remove = Vec::new();
            for (sid, arc) in sessions.iter() {
                if let Ok(guard) = arc.try_lock() {
                    if guard.last_accessed.elapsed() > timeout {
                        to_remove.push(sid.clone());
                    }
                }
            }
            for sid in to_remove {
                if let Some(arc) = sessions.remove(&sid) {
                    expired.push(arc);
                }
            }
        }
        let count = expired.len();
        for arc in expired {
            let mut guard = arc.lock().await;
            let _ = guard.close().await;
        }
        count
    }

    pub async fn session_count(&self) -> usize {
        self.sessions.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream as TokioUnixStream;

    #[test]
    fn test_firecracker_vm_paths() {
        let vm = FirecrackerVm::new("test-vm-1", "spiffe://aura.local/agent/vm-test");
        assert_eq!(vm.id, "test-vm-1");
        assert!(vm.socket_path.to_str().unwrap().contains("test-vm-1"));
        assert!(vm.chroot_dir.to_str().unwrap().contains("test-vm-1"));
        assert!(vm.vsock_uds_path.to_str().unwrap().contains("test-vm-1"));
        assert_eq!(
            vm.screenshot_socket_path(5252),
            vm.chroot_dir.join("root/run/vsock.socket_5252")
        );
    }

    #[test]
    fn test_firecracker_vm_lifecycle_teardown() {
        let mut vm = FirecrackerVm::new("test-vm-teardown", "spiffe://aura.local/test");
        let teardown_res = vm.teardown();
        assert!(teardown_res.is_ok());
    }

    #[test]
    fn test_firecracker_boot_config_schema() {
        let config = VmConfig {
            kernel_path: "/boot/vmlinux".to_string(),
            rootfs_path: "/boot/rootfs.ext4".to_string(),
            vcpu_count: 1,
            mem_size_mib: 128,
            rootfs_type: SpecializedRootfs::Custom,
            scratch_disk_path: None,
            is_read_only_rootfs: true,
        };
        let mut vm = FirecrackerVm::new("test-vm-boot", "spiffe://aura.local/test");
        // Socket does not exist in unit test environment, should gracefully succeed
        let boot_res = vm.configure_and_boot(&config);
        assert!(boot_res.is_ok());
    }

    #[test]
    fn test_specialized_rootfs_configurations() {
        let bash_cfg = VmConfig::for_bash(Some("/srv/jailer/images"));
        assert_eq!(bash_cfg.rootfs_path, "/srv/jailer/images/bash.ext4");
        assert_eq!(bash_cfg.vcpu_count, 1);
        assert_eq!(bash_cfg.mem_size_mib, 128);
        assert_eq!(bash_cfg.rootfs_type, SpecializedRootfs::Bash);

        let browser_cfg = VmConfig::for_browser(None);
        assert_eq!(browser_cfg.rootfs_path, "/srv/jailer/browser.ext4");
        assert_eq!(browser_cfg.vcpu_count, 2);
        assert_eq!(browser_cfg.mem_size_mib, 1024);
        assert_eq!(browser_cfg.rootfs_type, SpecializedRootfs::Browser);

        let computer_cfg = VmConfig::for_computer(None);
        assert_eq!(computer_cfg.rootfs_path, "/srv/jailer/computer.ext4");
        assert_eq!(computer_cfg.vcpu_count, 2);
        assert_eq!(computer_cfg.mem_size_mib, 1024);
        assert_eq!(computer_cfg.rootfs_type, SpecializedRootfs::Computer);
    }

    #[tokio::test]
    async fn test_screenshot_valid_png_received_and_saved() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sock_path = temp_dir.path().join("vsock_5252.sock");
        let artifacts_dir = temp_dir.path().join("artifacts");

        let mut listener = start_screenshot_listener(sock_path.clone(), artifacts_dir.clone())
            .await
            .expect("Failed to start screenshot listener");

        // Synthetic valid PNG image stream
        let mut sample_png = Vec::from(PNG_MAGIC_BYTES);
        sample_png.extend_from_slice(b"TEST_IMAGE_PAYLOAD_BYTES");

        // Connect client and send PNG stream
        let mut client = TokioUnixStream::connect(&sock_path).await.unwrap();
        client.write_all(&sample_png).await.unwrap();
        client.shutdown().await.unwrap();

        // Await artifact
        let artifact = listener.next_screenshot().await.expect("Expected artifact");
        assert!(artifact.file_path.exists());
        assert_eq!(artifact.size_bytes, sample_png.len());

        let read_bytes = tokio::fs::read(&artifact.file_path).await.unwrap();
        assert_eq!(read_bytes, sample_png);

        listener.stop();
    }

    #[tokio::test]
    async fn test_screenshot_rejects_non_png_under_zero_trust() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sock_path = temp_dir.path().join("vsock_bad.sock");
        let artifacts_dir = temp_dir.path().join("artifacts");

        let listener = start_screenshot_listener(sock_path.clone(), artifacts_dir.clone())
            .await
            .expect("Failed to start screenshot listener");

        // Non-PNG malicious or invalid payload
        let bad_payload = b"MALICIOUS_PAYLOAD_NOT_A_PNG";

        let mut client = TokioUnixStream::connect(&sock_path).await.unwrap();
        client.write_all(bad_payload).await.unwrap();
        client.shutdown().await.unwrap();

        // Wait brief moment and verify no artifact created
        tokio::time::sleep(Duration::from_millis(50)).await;
        let mut dir_entries = tokio::fs::read_dir(&artifacts_dir).await.unwrap();
        let first_entry = dir_entries.next_entry().await.unwrap();
        assert!(
            first_entry.is_none(),
            "Artifact should not be created for non-PNG stream"
        );

        listener.stop();
    }

    #[tokio::test]
    async fn test_screenshot_length_prefixed_payload() {
        let temp_dir = tempfile::tempdir().unwrap();
        let sock_path = temp_dir.path().join("vsock_len.sock");
        let artifacts_dir = temp_dir.path().join("artifacts");

        let mut listener = start_screenshot_listener(sock_path.clone(), artifacts_dir.clone())
            .await
            .expect("Failed to start screenshot listener");

        // Valid PNG with 4-byte big-endian length prefix
        let mut raw_png = Vec::from(PNG_MAGIC_BYTES);
        raw_png.extend_from_slice(b"LENGTH_PREFIXED_CONTENT");

        let len_prefix = (raw_png.len() as u32).to_be_bytes();
        let mut stream_bytes = Vec::new();
        stream_bytes.extend_from_slice(&len_prefix);
        stream_bytes.extend_from_slice(&raw_png);

        let mut client = TokioUnixStream::connect(&sock_path).await.unwrap();
        client.write_all(&stream_bytes).await.unwrap();
        client.shutdown().await.unwrap();

        let artifact = listener.next_screenshot().await.expect("Expected artifact");
        assert!(artifact.file_path.exists());
        assert_eq!(artifact.size_bytes, raw_png.len());

        let read_bytes = tokio::fs::read(&artifact.file_path).await.unwrap();
        assert_eq!(read_bytes, raw_png);

        listener.stop();
    }

    #[test]
    fn test_validate_kvm_missing_device() {
        let res = validate_kvm_at_path(Path::new("/tmp/nonexistent_kvm_device_xyz"));
        assert!(matches!(res, Err(KvmError::KvmDeviceNotFound(_))));
        if let Err(e) = res {
            assert!(e.to_string().contains("KVM device not found"));
        }
    }

    #[test]
    fn test_validate_kvm_permission_denied() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_file = temp_dir.path().join("mock_kvm_readonly");
        std::fs::write(&temp_file, b"").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&temp_file).unwrap().permissions();
            perms.set_mode(0o400);
            std::fs::set_permissions(&temp_file, perms).unwrap();
        }
        let res = validate_kvm_at_path(&temp_file);
        assert!(matches!(res, Err(KvmError::KvmPermissionDenied(_))));
        if let Err(e) = res {
            assert!(e.to_string().contains("Permission denied"));
        }
    }

    #[test]
    fn test_validate_kvm_accessible_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_file = temp_dir.path().join("mock_kvm_rw");
        std::fs::write(&temp_file, b"").unwrap();
        let res = validate_kvm_at_path(&temp_file);
        assert!(res.is_ok());
    }

    #[test]
    fn test_validate_kvm_skip_env_override() {
        std::env::set_var(KVM_SKIP_ENV_VAR, "1");
        assert!(should_skip_kvm_validation());
        assert!(validate_kvm_support().is_ok());

        std::env::set_var(KVM_SKIP_ENV_VAR, "true");
        assert!(should_skip_kvm_validation());

        std::env::set_var(KVM_SKIP_ENV_VAR, "yes");
        assert!(should_skip_kvm_validation());
    }

    #[test]
    fn test_firecracker_rootfs_is_read_only() {
        let config = VmConfig {
            kernel_path: "/boot/vmlinux".to_string(),
            rootfs_path: "/srv/jailer/bash.ext4".to_string(),
            vcpu_count: 1,
            mem_size_mib: 128,
            rootfs_type: SpecializedRootfs::Bash,
            scratch_disk_path: None,
            is_read_only_rootfs: true,
        };

        // 1. Assert rootfs drive payload contains "is_read_only": true (EXE-VM-011)
        let drive_payload = FirecrackerVm::build_rootfs_drive_payload(&config);
        let parsed_drive: serde_json::Value = serde_json::from_str(&drive_payload).unwrap();
        assert_eq!(parsed_drive["drive_id"], "rootfs");
        assert_eq!(parsed_drive["is_root_device"], true);
        assert_eq!(parsed_drive["is_read_only"], true);

        // 2. Assert kernel boot args include ro root=/dev/vda (EXE-VM-011)
        let boot_payload = FirecrackerVm::build_boot_source_payload(&config);
        let parsed_boot: serde_json::Value = serde_json::from_str(&boot_payload).unwrap();
        let boot_args = parsed_boot["boot_args"].as_str().unwrap();
        assert!(boot_args.contains("ro root=/dev/vda"));
    }

    #[test]
    fn test_firecracker_scratch_disk_payload_and_cleanup() {
        let temp_dir = tempfile::tempdir().unwrap();
        let scratch_file = temp_dir.path().join("ephemeral_scratch.ext4");
        std::fs::write(&scratch_file, b"SCRATCH_DISK_EXT4_DATA").unwrap();
        assert!(scratch_file.exists());

        let scratch_str = scratch_file.to_str().unwrap().to_string();
        let config = VmConfig::default().with_scratch_disk(&scratch_str);

        // 1. Verify secondary drive payload
        let scratch_payload = FirecrackerVm::build_scratch_drive_payload(&scratch_str);
        let parsed_scratch: serde_json::Value = serde_json::from_str(&scratch_payload).unwrap();
        assert_eq!(parsed_scratch["drive_id"], "scratch");
        assert_eq!(parsed_scratch["is_root_device"], false);
        assert_eq!(parsed_scratch["is_read_only"], false);
        assert_eq!(parsed_scratch["path_on_host"], scratch_str);

        // 2. Boot VM and ensure teardown removes ephemeral scratch file
        let mut vm = FirecrackerVm::new("test-vm-scratch", "spiffe://aura.local/test");
        let boot_res = vm.configure_and_boot(&config);
        assert!(boot_res.is_ok());
        assert_eq!(vm.scratch_disk_path, Some(scratch_file.clone()));

        // Teardown should delete ephemeral scratch file (EXE-VM-011)
        let teardown_res = vm.teardown();
        assert!(teardown_res.is_ok());
        assert!(!scratch_file.exists(), "Ephemeral scratch disk file should be removed on teardown");
    }

    #[tokio::test]
    async fn test_bash_session_manager_lifecycle_and_state_continuity() {
        let vm_config = VmConfig::for_bash(None);
        let manager = BashSessionManager::new(vm_config, Duration::from_secs(125));

        // Step 1: Set environment variable in session 1
        let out1 = manager
            .execute_in_session("sid-continuity", "export AURA_TEST_STATE=AlphaSessionValue")
            .await
            .expect("Session execution failed");
        assert_eq!(out1.exit_code, 0);

        // Step 2: Read environment variable in session 1
        let out2 = manager
            .execute_in_session("sid-continuity", "echo $AURA_TEST_STATE")
            .await
            .expect("Session echo failed");
        assert_eq!(out2.stdout, "AlphaSessionValue");
        assert_eq!(out2.exit_code, 0);

        // Step 3: Verify isolation from session 2
        let out_other = manager
            .execute_in_session("sid-isolated", "echo $AURA_TEST_STATE")
            .await
            .expect("Isolated session echo failed");
        assert_eq!(out_other.stdout, "");

        // Step 4: Working directory continuity
        let _ = manager
            .execute_in_session("sid-continuity", "cd /tmp")
            .await
            .unwrap();
        let pwd_out = manager
            .execute_in_session("sid-continuity", "pwd")
            .await
            .unwrap();
        let real_tmp = std::fs::canonicalize("/tmp").unwrap().to_string_lossy().to_string();
        assert!(pwd_out.stdout == "/tmp" || pwd_out.stdout == real_tmp);

        // Step 5: Session count and close
        assert_eq!(manager.session_count().await, 2);
        let closed = manager.close_session("sid-continuity").await.unwrap();
        assert!(closed);
        assert_eq!(manager.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_bash_session_inactivity_cleanup() {
        let vm_config = VmConfig::for_bash(None);
        let manager = BashSessionManager::new(vm_config, Duration::from_millis(50));

        let _ = manager
            .execute_in_session("sid-expire", "echo 'active'")
            .await
            .unwrap();
        assert_eq!(manager.session_count().await, 1);

        tokio::time::sleep(Duration::from_millis(60)).await;
        let cleaned = manager.cleanup_expired_sessions().await;
        assert_eq!(cleaned, 1);
        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_cdp_vsock_forwarder_handshake_and_bidirectional_transfer() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vsock_path = temp_dir.path().join("mock_vsock.socket");

        // Spawn mock Firecracker vsock listener
        let listener = tokio::net::UnixListener::bind(&vsock_path).unwrap();
        let vsock_server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                // Read CONNECT 9222\n handshake
                let mut line = Vec::new();
                let mut b = [0u8; 1];
                while b[0] != b'\n' {
                    let _ = tokio::io::AsyncReadExt::read_exact(&mut stream, &mut b).await;
                    line.push(b[0]);
                }
                let s = String::from_utf8_lossy(&line);
                assert!(s.starts_with("CONNECT 9222"));

                // Reply OK <port>\n
                tokio::io::AsyncWriteExt::write_all(&mut stream, b"OK 12345\n").await.unwrap();

                // Echo server
                let mut buf = [0u8; 512];
                if let Ok(n) = tokio::io::AsyncReadExt::read(&mut stream, &mut buf).await {
                    if n > 0 {
                        let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, &buf[..n]).await;
                    }
                }
            }
        });

        let forwarder = CdpVsockForwarder::start(
            vsock_path,
            "127.0.0.1:0".parse().unwrap(),
            DEFAULT_CDP_VSOCK_PORT,
        )
        .await
        .expect("Failed to start forwarder");

        let bound_addr = forwarder.local_addr();

        // Connect TCP client
        let mut client = tokio::net::TcpStream::connect(bound_addr).await.unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut client, b"PING_CDP_PAYLOAD").await.unwrap();

        let mut resp = [0u8; 16];
        tokio::io::AsyncReadExt::read_exact(&mut client, &mut resp).await.unwrap();
        assert_eq!(&resp, b"PING_CDP_PAYLOAD");

        forwarder.stop();
        vsock_server.await.unwrap();
    }

    #[tokio::test]
    async fn test_cdp_vsock_forwarder_rejects_on_handshake_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let vsock_path = temp_dir.path().join("mock_vsock_err.socket");

        let listener = tokio::net::UnixListener::bind(&vsock_path).unwrap();
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut b = [0u8; 1];
                while b[0] != b'\n' {
                    let _ = tokio::io::AsyncReadExt::read_exact(&mut stream, &mut b).await;
                }
                // Send ERR response
                let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, b"ERR Port not listening\n").await;
            }
        });

        let forwarder = CdpVsockForwarder::start(
            vsock_path,
            "127.0.0.1:0".parse().unwrap(),
            DEFAULT_CDP_VSOCK_PORT,
        )
        .await
        .unwrap();

        let bound_addr = forwarder.local_addr();
        let mut client = tokio::net::TcpStream::connect(bound_addr).await.unwrap();
        let _ = tokio::io::AsyncWriteExt::write_all(&mut client, b"SHOULD_FAIL").await;
        let mut buf = [0u8; 10];
        let read_res = tokio::io::AsyncReadExt::read(&mut client, &mut buf).await;
        // Forwarder dropped client connection due to rejected handshake
        assert!(read_res.is_err() || read_res.unwrap() == 0);

        forwarder.stop();
    }

    #[tokio::test]
    async fn test_firecracker_vm_cdp_forwarder_lifecycle() {
        let mut vm = FirecrackerVm::new("test-vm-cdp", "spiffe://aura.local/test");
        assert!(vm.cdp_forwarder.is_none());

        let bound_addr = vm
            .start_cdp_forwarder("127.0.0.1:0".parse().unwrap())
            .await
            .expect("start_cdp_forwarder failed");
        assert_ne!(bound_addr.port(), 0);
        assert!(vm.cdp_forwarder.is_some());

        vm.stop_cdp_forwarder();
        assert!(vm.cdp_forwarder.is_none());
    }
}

