#!/usr/bin/env bash
# Aura OS - Firecracker RootFS Common Image Build Utilities
# Copyright (c) 2026 Jeff Kenniston. Apache-2.0 License.

set -euo pipefail

# ANSI color codes
readonly COLOR_RED='\033[0;31m'
readonly COLOR_GREEN='\033[0;32m'
readonly COLOR_YELLOW='\033[1;33m'
readonly COLOR_BLUE='\033[0;34m'
readonly COLOR_CYAN='\033[0;36m'
readonly COLOR_RESET='\033[0m'

log_info() {
    printf "${COLOR_BLUE}[INFO]${COLOR_RESET} %s\n" "$*" >&2
}

log_step() {
    printf "${COLOR_CYAN}[STEP]${COLOR_RESET} %s\n" "$*" >&2
}

log_success() {
    printf "${COLOR_GREEN}[SUCCESS]${COLOR_RESET} %s\n" "$*" >&2
}

log_warn() {
    printf "${COLOR_YELLOW}[WARN]${COLOR_RESET} %s\n" "$*" >&2
}

log_error() {
    printf "${COLOR_RED}[ERROR]${COLOR_RESET} %s\n" "$*" >&2
}

die() {
    log_error "$*"
    exit 1
}

# Verifies that required host utilities are installed
check_prerequisites() {
    local missing=()
    for cmd in truncate mkfs.ext4 curl tar sudo chroot; do
        if ! command -v "$cmd" >/dev/null 2>&1; then
            missing+=("$cmd")
        fi
    done

    if [ ${#missing[@]} -gt 0 ]; then
        die "Missing required command-line tools: ${missing[*]}"
    fi

    # Verify sudo privileges
    if ! sudo -n true 2>/dev/null; then
        log_warn "sudo password may be required to format and mount ext4 loopback images."
    fi
}

CLEANUP_MOUNT_DIR=""
CLEANUP_WORK_DIR=""

cleanup() {
    local exit_code=$?
    log_info "Running cleanup handlers..."
    if [ -n "${CLEANUP_MOUNT_DIR:-}" ] && [ -d "$CLEANUP_MOUNT_DIR" ]; then
        # Unmount pseudo filesystems in reverse order
        for m in "$CLEANUP_MOUNT_DIR/dev/pts" "$CLEANUP_MOUNT_DIR/dev" "$CLEANUP_MOUNT_DIR/proc" "$CLEANUP_MOUNT_DIR/sys"; do
            if mountpoint -q "$m" 2>/dev/null; then
                sudo umount -f "$m" 2>/dev/null || sudo umount -l "$m" 2>/dev/null || true
            fi
        done
        if mountpoint -q "$CLEANUP_MOUNT_DIR" 2>/dev/null; then
            sudo umount -f "$CLEANUP_MOUNT_DIR" 2>/dev/null || sudo umount -l "$CLEANUP_MOUNT_DIR" 2>/dev/null || true
        fi
    fi
    if [ -n "${CLEANUP_WORK_DIR:-}" ] && [ -d "$CLEANUP_WORK_DIR" ]; then
        sudo rm -rf "$CLEANUP_WORK_DIR" 2>/dev/null || true
    fi
    if [ $exit_code -ne 0 ]; then
        log_error "Build process exited with code $exit_code"
    fi
    exit $exit_code
}

# Sets up a cleanup trap on script exit
setup_cleanup_trap() {
    CLEANUP_MOUNT_DIR="$1"
    CLEANUP_WORK_DIR="$2"
    trap cleanup EXIT INT TERM
}

# Unmounts all guest filesystems cleanly before final fsck
unmount_guest_filesystems() {
    local mount_dir="$1"
    log_step "Unmounting guest filesystem mountpoints..."
    sync

    for m in "$mount_dir/dev/pts" "$mount_dir/dev" "$mount_dir/proc" "$mount_dir/sys"; do
        if mountpoint -q "$m" 2>/dev/null; then
            sudo umount "$m" 2>/dev/null || sudo umount -l "$m" 2>/dev/null || true
        fi
    done

    if mountpoint -q "$mount_dir" 2>/dev/null; then
        sudo umount "$mount_dir"
    fi
    CLEANUP_MOUNT_DIR=""
}

# Creates a formatted ext4 sparse disk image
create_ext4_image() {
    local image_path="$1"
    local size_mb="$2"
    local label="$3"

    log_step "Creating ext4 image: $image_path (${size_mb}MB, label='$label')..."
    mkdir -p "$(dirname "$image_path")"
    rm -f "$image_path"

    truncate -s "${size_mb}M" "$image_path"
    # Format ext4 with 4096 block size and no resize inode overhead for Firecracker
    mkfs.ext4 -F -L "$label" -b 4096 "$image_path" >/dev/null
    log_info "Ext4 filesystem created successfully."
}

# Downloads and caches Alpine minirootfs tarball
download_alpine_minirootfs() {
    local cache_dir="$1"
    local version="$2"
    local tarball_name="alpine-minirootfs-${version}-x86_64.tar.gz"
    local cached_tarball="${cache_dir}/${tarball_name}"
    local download_url="https://dl-cdn.alpinelinux.org/alpine/v3.20/releases/x86_64/${tarball_name}"

    mkdir -p "$cache_dir"
    if [ ! -f "$cached_tarball" ]; then
        log_info "Downloading Alpine Linux minirootfs ($version)..."
        curl -fSL "$download_url" -o "$cached_tarball.tmp" || {
            # Try latest releases directory if explicit release fails
            local fallback_url="https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.20.0-x86_64.tar.gz"
            log_warn "Primary URL failed, attempting fallback: $fallback_url"
            curl -fSL "$fallback_url" -o "$cached_tarball.tmp" || die "Failed to download Alpine minirootfs"
        }
        mv "$cached_tarball.tmp" "$cached_tarball"
    else
        log_info "Using cached Alpine minirootfs: $cached_tarball"
    fi

    echo "$cached_tarball"
}

# Mounts guest pseudo-filesystems for chroot commands
mount_guest_pseudo_fs() {
    local mount_dir="$1"
    log_info "Mounting proc, sys, dev for chroot environment..."
    sudo mkdir -p "$mount_dir/proc" "$mount_dir/sys" "$mount_dir/dev" "$mount_dir/dev/pts"
    sudo mount -t proc proc "$mount_dir/proc"
    sudo mount -t sysfs sys "$mount_dir/sys"
    sudo mount --bind /dev "$mount_dir/dev"
    sudo mount -t devpts devpts "$mount_dir/dev/pts"
}

# Sets up DNS resolution and base networking in guest
setup_guest_network_config() {
    local mount_dir="$1"
    local hostname="$2"

    log_info "Configuring network resolution and hostname for $hostname..."
    if [ -f /etc/resolv.conf ]; then
        sudo cp /etc/resolv.conf "$mount_dir/etc/resolv.conf"
        echo "nameserver 1.1.1.1" | sudo tee -a "$mount_dir/etc/resolv.conf" >/dev/null || true
        echo "nameserver 8.8.8.8" | sudo tee -a "$mount_dir/etc/resolv.conf" >/dev/null || true
    else
        sudo tee "$mount_dir/etc/resolv.conf" >/dev/null <<'EOF'
nameserver 1.1.1.1
nameserver 8.8.8.8
EOF
    fi

    echo "$hostname" | sudo tee "$mount_dir/etc/hostname" >/dev/null

    sudo tee "$mount_dir/etc/hosts" >/dev/null <<EOF
127.0.0.1   localhost $hostname
::1         localhost ip6-localhost ip6-loopback
EOF
}

# Sets up /etc/fstab for Firecracker root and memory filesystems
setup_guest_fstab() {
    local mount_dir="$1"
    log_info "Configuring /etc/fstab..."
    sudo tee "$mount_dir/etc/fstab" >/dev/null <<'EOF'
/dev/vda        /               ext4    noatime,errors=remount-ro   0 1
devtmpfs        /dev            devtmpfs defaults                   0 0
proc            /proc           proc    defaults                    0 0
sysfs           /sys            sysfs   defaults                    0 0
tmpfs           /dev/shm        tmpfs   defaults,size=512M          0 0
tmpfs           /tmp            tmpfs   defaults                    0 0
tmpfs           /run            tmpfs   defaults                    0 0
EOF
}

# Configures Alpine repositories to include main and community
configure_alpine_repos() {
    local mount_dir="$1"
    log_info "Configuring Alpine package repositories (main + community)..."
    sudo tee "$mount_dir/etc/apk/repositories" >/dev/null <<'EOF'
https://dl-cdn.alpinelinux.org/alpine/v3.20/main
https://dl-cdn.alpinelinux.org/alpine/v3.20/community
EOF
}
