# Aura OS — Specialized Firecracker MicroVM RootFS Images

This directory provides the infrastructure-as-code scripts and specialized daemons to build production-grade `ext4` rootfs images for Firecracker microVM execution sandboxes in Aura OS (Phase M2).

## Overview

Aura OS enforces strict Docker-free sandboxing (**ADR-001**, **EXE-VM**). Computational tasks run as WASI 0.3 WebAssembly components, while heavyweight tasks execute inside session-scoped, ephemeral Firecracker microVMs booted in under 125ms via KVM and isolated by `jailer` (cgroups v2 + seccomp).

This module delivers:
1. **`build_browser_image.sh`** (`TOOL-BRW-001`): Pre-bundles headless Chromium and font configurations for sub-125ms cold boot execution.
2. **`build_computer_image.sh`** (`TOOL-CMP-001`, `TOOL-CMP-003`): Configures `Xvfb` virtual framebuffer, `fluxbox` window manager, GUI testing tools (`xcalc`, `xdotool`), and compiles a dedicated Rust AF_VSOCK screenshot streaming daemon.
3. **`vsock-screenshot-daemon`**: A lightweight, pure-Rust daemon running inside the guest that captures X11 framebuffers and streams PNG bytes across `AF_VSOCK` to the host supervisor.

---

## Directory Structure

```
deploy/images/
├── build_browser_image.sh       # Script to build browser.ext4 image
├── build_computer_image.sh      # Script to build computer.ext4 image
├── common.sh                    # Shared image build and cleanup routines
├── vsock-screenshot-daemon/     # Rust AF_VSOCK screenshot streaming daemon
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              # CLI entry point and modes
│       ├── capture.rs           # Pure-Rust X11 framebuffer capture & PNG encoder
│       └── server.rs            # AF_VSOCK listener and client push logic
├── output/                      # Generated ext4 images (default output)
└── README.md
```

---

## Prerequisites

- **Host OS**: Linux x86_64 with KVM support (`/dev/kvm`, `/dev/vhost-vsock`)
- **Host Tools**: `truncate`, `mkfs.ext4`, `curl`, `tar`, `sudo`, `chroot`
- **Privileges**: `sudo` access to create loop devices and mount filesystem images.

---

## 1. Browser Image Builder (`build_browser_image.sh`)

Generates `browser.ext4` pre-bundling headless Chromium with all shared libraries and font configurations to eliminate dynamic network dependencies during tool execution.

### Usage

```bash
# Build with default parameters (Alpine Linux base, 1536 MiB image)
sudo ./deploy/images/build_browser_image.sh

# Custom image output and size
sudo ./deploy/images/build_browser_image.sh --output /srv/jailer/browser.ext4 --size 2048

# Available options
./deploy/images/build_browser_image.sh --help
```

### Options

| Option | Description | Default |
|---|---|---|
| `-o, --output PATH` | Target ext4 output file | `deploy/images/output/browser.ext4` |
| `-s, --size MB` | Image disk size in MiB | `1536` |
| `-d, --distro DISTRO` | Base distribution (`alpine` or `debian`) | `alpine` |
| `--alpine-version VER` | Alpine minirootfs version | `3.20.10` |
| `--no-test` | Skip in-chroot Chromium smoke test | `false` |
| `--cache-dir DIR` | Tarball cache directory | `deploy/images/.cache` |

### Key Features

- **Pre-cached Font Configurations**: Executes `fc-cache -fv` at build time to prevent font scan pauses during guest startup.
- **Dedicated Unprivileged Profile**: Pre-configures user `aura` (UID 1000, GID 1000) for running browser processes.
- **Fast Init**: Custom `/sbin/init` mounts pseudo-filesystems (`/dev/shm`, `/proc`, `/sys`) in under 20ms and brings up the loopback interface.
- **CDP Remote Debugging**: Installs `/usr/local/bin/aura-browser` pre-configured for headless Chrome DevTools Protocol on port `9222`.

---

## 2. Computer Image Builder (`build_computer_image.sh`)

Generates `computer.ext4` pre-configured with a virtual display server and AF_VSOCK screenshot streaming daemon.

### Usage

```bash
# Build with default parameters (Alpine Linux base, 1024x768x24 resolution)
sudo ./deploy/images/build_computer_image.sh

# Custom resolution and vsock port
sudo ./deploy/images/build_computer_image.sh --screen-res 1280x800x24 --vsock-port 5254

# Available options
./deploy/images/build_computer_image.sh --help
```

### Options

| Option | Description | Default |
|---|---|---|
| `-o, --output PATH` | Target ext4 output file | `deploy/images/output/computer.ext4` |
| `-s, --size MB` | Image disk size in MiB | `1200` |
| `-d, --distro DISTRO` | Base distribution (`alpine` or `debian`) | `alpine` |
| `--screen-res RES` | Xvfb screen resolution (`WIDTHxHEIGHTxDEPTH`) | `1024x768x24` |
| `--vsock-port PORT` | AF_VSOCK port for screenshot listener | `5254` |
| `--no-test` | Skip in-chroot daemon smoke test | `false` |

### Key Features

- **Virtual Display**: Pre-configures `Xvfb` on `DISPLAY=:0` with RANDR and RENDER extensions.
- **Minimal Window Manager**: Pre-installs `fluxbox` and `openbox` for fast, lightweight window rendering.
- **UI Test Utilities**: Pre-bundles `xcalc` (for visual validation) and `xdotool` (for simulated keyboard/pointer inputs).
- **Embedded Rust Daemon**: Automatically compiles and installs `vsock-screenshot-daemon` inside the rootfs.
- **Auto-start**: `/sbin/init` automatically boots the display stack (`start-desktop`) during Firecracker guest init.

---

## 3. Rust AF_VSOCK Screenshot Daemon (`vsock-screenshot-daemon`)

The daemon bridges guest X11 graphical frames to the host `aura-core` supervisor using Linux `AF_VSOCK` sockets.

### Architecture

```
Host (aura-core)                      Firecracker Guest (computer.ext4)
┌────────────────────────┐            ┌─────────────────────────────────────────┐
│ Hypervisor / Supervisor│            │ Xvfb :0 ──> fluxbox ──> GUI Apps (xcalc)│
│                        │            │   │                                     │
│ Unix Domain Socket     │  VSOCK IPC │   ▼                                     │
│ (/run/vsock.socket)    │ <════════> │ vsock-screenshot-daemon                 │
│                        │            │   - Connects to X11 socket              │
│ Screenshot Artifact    │            │   - Captures root pixmap via x11rb      │
│ (artifacts/frame.png)  │            │   - Encodes RGBA -> PNG buffer          │
└────────────────────────┘            │   - Streams PNG bytes over port 5254    │
                                      └─────────────────────────────────────────┘
```

### Modes of Operation

1. **Listener Daemon (Default)**:
   ```bash
   vsock-screenshot-daemon --listen --port 5254 --display :0
   ```
   Accepts connections from host over Firecracker VSOCK. Upon connection, captures the current X11 screen and streams the PNG image.

2. **Push Client Mode**:
   ```bash
   vsock-screenshot-daemon --connect-host 5254 --display :0
   ```
   Connects to host CID 2 (`VMADDR_CID_HOST`) and pipes the current frame.

3. **One-Shot Local Capture**:
   ```bash
   vsock-screenshot-daemon --output /tmp/screenshot.png --display :0
   ```

4. **Stdout Stream**:
   ```bash
   vsock-screenshot-daemon --stdout --display :0 > frame.png
   ```

---

## Firecracker microVM Boot Example

To boot either image using Firecracker:

```bash
# Set boot-source with serial console
curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/boot-source' \
  -H 'Content-Type: application/json' \
  -d '{
    "kernel_image_path": "/srv/jailer/vmlinux.bin",
    "boot_args": "console=ttyS0 reboot=k panic=1 pci=off"
  }'

# Mount rootfs drive
curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/drives/rootfs' \
  -H 'Content-Type: application/json' \
  -d '{
    "drive_id": "rootfs",
    "path_on_host": "/path/to/deploy/images/output/computer.ext4",
    "is_root_device": true,
    "is_read_only": false
  }'

# Configure AF_VSOCK device (guest CID 3)
curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/vsock' \
  -H 'Content-Type: application/json' \
  -d '{
    "guest_cid": 3,
    "uds_path": "/run/vsock.socket",
    "vsock_id": "vsock0"
  }'

# Start instance (<125ms cold boot)
curl --unix-socket /tmp/firecracker.socket -X PUT 'http://localhost/actions' \
  -H 'Content-Type: application/json' \
  -d '{"action_type": "InstanceStart"}'
```

To capture a screenshot from the host:
```bash
# Connect to Firecracker vsock UDS and issue CONNECT 5254
nc -U /run/vsock.socket <<EOF > screenshot.png
CONNECT 5254
EOF
```
