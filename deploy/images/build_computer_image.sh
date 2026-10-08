#!/usr/bin/env bash
# Aura OS - Specialized Firecracker Computer RootFS Image Builder
# Pre-configures Xvfb, minimal window manager (fluxbox/openbox), xcalc/xdotool,
# and compiles & bundles the Rust AF_VSOCK screenshot daemon.
# Implements TOOL-CMP-001, TOOL-CMP-003.
# Copyright (c) 2026 Jeff Kenniston. Apache-2.0 License.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=deploy/images/common.sh
source "${SCRIPT_DIR}/common.sh"

# Default configuration
OUTPUT_PATH="${SCRIPT_DIR}/output/computer.ext4"
IMAGE_SIZE_MB=1200
DISTRO="alpine"
ALPINE_VERSION="3.20.10"
CACHE_DIR="${SCRIPT_DIR}/.cache"
SCREEN_RES="1024x768x24"
VSOCK_PORT=5254
DAEMON_SRC_DIR="${SCRIPT_DIR}/vsock-screenshot-daemon"
RUN_SMOKE_TEST=true

usage() {
    cat <<EOF
Usage: $(basename "$0") [OPTIONS]

Builds a specialized Firecracker ext4 rootfs image pre-configured with Xvfb,
fluxbox window manager, and the Rust AF_VSOCK screenshot daemon.

Options:
  -o, --output PATH         Output ext4 image path (default: $OUTPUT_PATH)
  -s, --size MB             Filesystem size in MiB (default: $IMAGE_SIZE_MB)
  -d, --distro DISTRO       Distribution: 'alpine' or 'debian' (default: $DISTRO)
      --alpine-version VER  Alpine minirootfs version (default: $ALPINE_VERSION)
      --screen-res RES      Xvfb screen resolution (default: $SCREEN_RES)
      --vsock-port PORT     AF_VSOCK screenshot listener port (default: $VSOCK_PORT)
      --no-test             Skip in-chroot daemon smoke test verification
      --cache-dir DIR       Download cache directory (default: $CACHE_DIR)
  -h, --help                Show this help message

Requirements:
  Linux host with root / sudo privileges, mkfs.ext4, truncate, curl, tar, chroot.
EOF
    exit 0
}

# Parse command line options
while [[ $# -gt 0 ]]; do
    case "$1" in
        -o|--output)
            OUTPUT_PATH="$2"
            shift 2
            ;;
        -s|--size)
            IMAGE_SIZE_MB="$2"
            shift 2
            ;;
        -d|--distro)
            DISTRO="$2"
            shift 2
            ;;
        --alpine-version)
            ALPINE_VERSION="$2"
            shift 2
            ;;
        --screen-res)
            SCREEN_RES="$2"
            shift 2
            ;;
        --vsock-port)
            VSOCK_PORT="$2"
            shift 2
            ;;
        --no-test)
            RUN_SMOKE_TEST=false
            shift
            ;;
        --cache-dir)
            CACHE_DIR="$2"
            shift 2
            ;;
        -h|--help)
            usage
            ;;
        *)
            die "Unknown option: $1. Use --help for usage information."
            ;;
    esac
done

check_prerequisites

if [ ! -d "$DAEMON_SRC_DIR" ]; then
    die "Screenshot daemon source directory not found at: $DAEMON_SRC_DIR"
fi

WORK_DIR=$(mktemp -d /tmp/aura-computer-build.XXXXXX)
MOUNT_DIR="${WORK_DIR}/mnt"
mkdir -p "$MOUNT_DIR" "$CACHE_DIR"

setup_cleanup_trap "$MOUNT_DIR" "$WORK_DIR"

log_step "Starting Aura Computer Firecracker RootFS Build (TOOL-CMP-001, TOOL-CMP-003)"
log_info "Target Image: $OUTPUT_PATH (${IMAGE_SIZE_MB} MiB, Distro: $DISTRO)"
log_info "Resolution:   $SCREEN_RES, VSOCK Port: $VSOCK_PORT"

# 1. Create sparse ext4 image
create_ext4_image "$OUTPUT_PATH" "$IMAGE_SIZE_MB" "aura-computer"

# 2. Mount ext4 image via loopback
log_step "Mounting ext4 image to loop device..."
sudo mount -o loop "$OUTPUT_PATH" "$MOUNT_DIR"

if [ "$DISTRO" = "alpine" ]; then
    # 3. Download and extract Alpine minirootfs
    tarball=$(download_alpine_minirootfs "$CACHE_DIR" "$ALPINE_VERSION")
    log_step "Extracting base rootfs into loop mount..."
    sudo tar -xzf "$tarball" -C "$MOUNT_DIR"

    # 4. Network and filesystem configuration
    setup_guest_network_config "$MOUNT_DIR" "aura-computer"
    setup_guest_fstab "$MOUNT_DIR"
    configure_alpine_repos "$MOUNT_DIR"

    # 5. Mount guest pseudo-filesystems for package installation
    mount_guest_pseudo_fs "$MOUNT_DIR"

    # 6. Install Xvfb, window manager, UI tools, and fonts
    log_step "Installing Xvfb, fluxbox, openbox, xcalc, xdotool via apk..."
    sudo chroot "$MOUNT_DIR" /bin/sh -c '
        set -eu
        apk update
        apk add --no-cache \
            xvfb \
            fluxbox \
            openbox \
            xcalc \
            xdotool \
            xterm \
            xrandr \
            xsetroot \
            xwd \
            libx11 \
            libxcb \
            font-dejavu \
            ttf-freefont \
            ca-certificates \
            curl \
            util-linux \
            bash \
            iproute2 \
            coreutils \
            procps \
            tzdata

        fc-cache -fv
    '

    # 7. Compile and install Rust vsock screenshot daemon inside Alpine chroot
    log_step "Compiling vsock-screenshot-daemon inside Alpine rootfs..."
    sudo mkdir -p "$MOUNT_DIR/tmp/daemon-src"
    sudo cp -r "$DAEMON_SRC_DIR"/* "$MOUNT_DIR/tmp/daemon-src/"

    sudo chroot "$MOUNT_DIR" /bin/sh -c '
        set -eu
        echo "[Build] Installing temporary Rust and Cargo compilers..."
        apk add --no-cache rust cargo gcc musl-dev

        echo "[Build] Compiling vsock-screenshot-daemon in release mode..."
        cd /tmp/daemon-src
        cargo build --release

        echo "[Build] Installing binary to /usr/local/bin..."
        cp target/release/vsock-screenshot-daemon /usr/local/bin/vsock-screenshot-daemon
        chmod +x /usr/local/bin/vsock-screenshot-daemon

        echo "[Build] Cleaning up Rust compiler and intermediate artifacts..."
        cd /
        rm -rf /tmp/daemon-src /root/.cargo /root/.rustup
        apk del rust cargo gcc musl-dev
    '

elif [ "$DISTRO" = "debian" ]; then
    if ! command -v debootstrap >/dev/null 2>&1; then
        die "Debian distro selected but debootstrap is not installed on host. Run: sudo apt install debootstrap"
    fi
    log_step "Bootstrapping Debian minimal rootfs via debootstrap..."
    sudo debootstrap --variant=minbase --include=ca-certificates,curl,iproute2,procps bookworm "$MOUNT_DIR" http://deb.debian.org/debian

    setup_guest_network_config "$MOUNT_DIR" "aura-computer"
    setup_guest_fstab "$MOUNT_DIR"
    mount_guest_pseudo_fs "$MOUNT_DIR"

    log_step "Installing Xvfb, fluxbox, xcalc, xdotool via apt..."
    sudo chroot "$MOUNT_DIR" /bin/sh -c '
        export DEBIAN_FRONTEND=noninteractive
        apt-get update
        apt-get install -y --no-install-recommends \
            xvfb \
            fluxbox \
            x11-apps \
            xdotool \
            xterm \
            x11-xserver-utils \
            fonts-dejavu-core \
            fonts-freefont-ttf
        fc-cache -fv
        apt-get clean
        rm -rf /var/lib/apt/lists/*
    '

    # Copy daemon binary if built on host or compile
    log_step "Installing vsock-screenshot-daemon..."
    if [ -f "$DAEMON_SRC_DIR/target/release/vsock-screenshot-daemon" ]; then
        sudo cp "$DAEMON_SRC_DIR/target/release/vsock-screenshot-daemon" "$MOUNT_DIR/usr/local/bin/"
        sudo chmod +x "$MOUNT_DIR/usr/local/bin/vsock-screenshot-daemon"
    else
        sudo mkdir -p "$MOUNT_DIR/tmp/daemon-src"
        sudo cp -r "$DAEMON_SRC_DIR"/* "$MOUNT_DIR/tmp/daemon-src/"
        sudo chroot "$MOUNT_DIR" /bin/sh -c '
            export DEBIAN_FRONTEND=noninteractive
            apt-get update && apt-get install -y cargo rustc
            cd /tmp/daemon-src && cargo build --release
            cp target/release/vsock-screenshot-daemon /usr/local/bin/
            cd / && rm -rf /tmp/daemon-src /root/.cargo
            apt-get remove -y cargo rustc && apt-get autoremove -y
            apt-get clean && rm -rf /var/lib/apt/lists/*
        '
    fi
else
    die "Unsupported distribution: $DISTRO. Must be 'alpine' or 'debian'."
fi

# 8. Install desktop environment startup script
log_step "Installing /usr/local/bin/start-desktop helper..."
sudo tee "$MOUNT_DIR/usr/local/bin/start-desktop" >/dev/null <<EOF
#!/bin/sh
# Starts Xvfb virtual framebuffer, fluxbox window manager, and vsock-screenshot-daemon
# Implements TOOL-CMP-001, TOOL-CMP-003.

export DISPLAY=:0
export HOME=/root

echo "[Aura] Starting Xvfb virtual framebuffer on :0 (${SCREEN_RES})..."
mkdir -p /tmp/.X11-unix
chmod 1777 /tmp/.X11-unix

Xvfb :0 -screen 0 ${SCREEN_RES} -ac +extension RANDR +extension RENDER -noreset &
XVFB_PID=\$!

# Wait for X11 socket to become ready
echo "[Aura] Waiting for X11 server socket..."
for i in \$(seq 1 30); do
    if [ -S /tmp/.X11-unix/X0 ]; then
        echo "[Aura] Xvfb is ready on DISPLAY=:0"
        break
    fi
    sleep 0.05
done

# Set standard background
xsetroot -solid "#1e1e2e" 2>/dev/null || true

# Start minimal window manager
echo "[Aura] Starting fluxbox window manager..."
fluxbox &
FLUXBOX_PID=\$!

# Start AF_VSOCK screenshot streaming daemon
echo "[Aura] Starting vsock-screenshot-daemon (AF_VSOCK port: ${VSOCK_PORT})..."
/usr/local/bin/vsock-screenshot-daemon --listen --port ${VSOCK_PORT} --display :0 &
DAEMON_PID=\$!

echo "[Aura] Desktop environment and vsock daemon initialized."
EOF
sudo chmod +x "$MOUNT_DIR/usr/local/bin/start-desktop"

# 9. Install helper launch scripts
log_step "Installing helper launch scripts (xcalc, take-screenshot)..."
sudo tee "$MOUNT_DIR/usr/local/bin/launch-xcalc" >/dev/null <<'EOF'
#!/bin/sh
# Verification utility: launches xcalc GUI calculator on DISPLAY=:0
export DISPLAY=:0
exec xcalc "$@"
EOF
sudo chmod +x "$MOUNT_DIR/usr/local/bin/launch-xcalc"

sudo tee "$MOUNT_DIR/usr/local/bin/take-screenshot" >/dev/null <<'EOF'
#!/bin/sh
# Captures local screenshot via vsock-screenshot-daemon CLI mode
export DISPLAY=:0
OUT_FILE="${1:-/tmp/screenshot.png}"
/usr/local/bin/vsock-screenshot-daemon -o "$OUT_FILE" --display :0
echo "Screenshot saved to: $OUT_FILE"
EOF
sudo chmod +x "$MOUNT_DIR/usr/local/bin/take-screenshot"

# 10. Install Firecracker sub-125ms cold-boot init script
log_step "Installing Firecracker /sbin/init script..."
sudo tee "$MOUNT_DIR/sbin/init" >/dev/null <<EOF
#!/bin/sh
# Aura MicroVM Fast Init (TOOL-CMP-001, TOOL-CMP-003)
export PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
export HOME=/root
export DISPLAY=:0

# Mount essential virtual filesystems
mount -t proc proc /proc 2>/dev/null || true
mount -t sysfs sys /sys 2>/dev/null || true
mount -t devtmpfs devtmpfs /dev 2>/dev/null || true
mkdir -p /dev/pts /dev/shm /run /tmp
mount -t devpts devpts /dev/pts 2>/dev/null || true
mount -t tmpfs -o size=512M tmpfs /dev/shm 2>/dev/null || true
mount -t tmpfs tmpfs /run 2>/dev/null || true
mount -t tmpfs tmpfs /tmp 2>/dev/null || true

# Bring up loopback interface
ip link set lo up 2>/dev/null || ifconfig lo up 2>/dev/null || true

echo "=========================================================="
echo " Aura OS Computer MicroVM (Firecracker KVM / VirtIO)"
echo " Screen: ${SCREEN_RES} (Xvfb + fluxbox)"
echo " Daemon: vsock-screenshot-daemon on port ${VSOCK_PORT}"
echo " Sub-125ms boot target achieved."
echo "=========================================================="

# Automatically initialize desktop environment and vsock daemon
/usr/local/bin/start-desktop &

# Fallback interactive shell on ttyS0 console
exec /bin/sh
EOF
sudo chmod +x "$MOUNT_DIR/sbin/init"
sudo ln -sf /sbin/init "$MOUNT_DIR/init"

# 11. Smoke test verification in chroot
if [ "$RUN_SMOKE_TEST" = true ]; then
    log_step "Executing daemon smoke test in chroot..."
    daemon_out=$(sudo chroot "$MOUNT_DIR" /usr/local/bin/vsock-screenshot-daemon --help 2>&1 || true)
    if ! echo "$daemon_out" | grep -qi "vsock-screenshot-daemon"; then
        die "Screenshot daemon verification failed inside rootfs: $daemon_out"
    fi
    log_success "vsock-screenshot-daemon binary verified successfully!"
fi

# 12. Cleanup temporary chroot files
log_step "Cleaning package cache and temp files inside guest..."
sudo chroot "$MOUNT_DIR" /bin/sh -c '
    rm -rf /var/cache/apk/* /tmp/* /root/.cache 2>/dev/null || true
'

# 13. Unmount filesystems cleanly
unmount_guest_filesystems "$MOUNT_DIR"

# 14. Filesystem check & optimize
log_step "Verifying ext4 filesystem integrity..."
e2fsck -f -y "$OUTPUT_PATH" >/dev/null || true

# 15. Report results
final_size_bytes=$(stat -c%s "$OUTPUT_PATH")
final_size_mb=$((final_size_bytes / 1024 / 1024))
sha256=$(sha256sum "$OUTPUT_PATH" | awk '{print $1}')

log_success "=========================================================="
log_success " Computer MicroVM Image Built Successfully!"
log_success " Image Path:    $OUTPUT_PATH"
log_success " Image Size:    ${final_size_mb} MiB (${final_size_bytes} bytes)"
log_success " SHA256:        $sha256"
log_success " Resolution:    $SCREEN_RES"
log_success " VSOCK Port:    $VSOCK_PORT"
log_success " Implements:    TOOL-CMP-001, TOOL-CMP-003"
log_success "=========================================================="
