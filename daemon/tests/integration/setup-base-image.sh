#!/bin/bash
# Create the base image with nix pre-installed
# Run once, then cached as "mac-mgmt-base" image

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/lib.sh"

IMAGE_NAME="$BASE_IMAGE"
BUILD_CONTAINER="mac-mgmt-base-build"

echo "Building base image: $IMAGE_NAME"

# Clean up any previous build container
incus delete --force "$BUILD_CONTAINER" 2>/dev/null || true

# Launch a fresh Ubuntu 24.04 container
echo "Launching Ubuntu 24.04 container..."
incus launch images:ubuntu/24.04 "$BUILD_CONTAINER" -c limits.memory=4GiB

# Wait for container to be ready
echo "Waiting for container to be ready..."
sleep 5
retries=30
while [ $retries -gt 0 ]; do
    if incus exec "$BUILD_CONTAINER" -- true 2>/dev/null; then
        break
    fi
    sleep 2
    retries=$((retries - 1))
done

# Wait for networking
echo "Waiting for networking..."
retries=30
while [ $retries -gt 0 ]; do
    if incus exec "$BUILD_CONTAINER" -- ping -c1 -W1 cache.nixos.org &>/dev/null; then
        break
    fi
    sleep 2
    retries=$((retries - 1))
done

# Install nix
echo "Installing nix..."
incus exec "$BUILD_CONTAINER" -- bash -c '
    apt-get update -qq && apt-get install -y -qq curl xz-utils > /dev/null 2>&1
    yes | sh <(curl --proto "=https" --tlsv1.2 -L https://nixos.org/nix/install) --daemon
'

# Configure nix (trusted-user + flakes; substituters are provided dynamically by the server)
echo "Configuring nix..."
incus exec "$BUILD_CONTAINER" -- bash -c '
    echo "extra-experimental-features = nix-command flakes" | tee -a /etc/nix/nix.conf
    echo "extra-trusted-users = root" | tee -a /etc/nix/nix.conf
    systemctl restart nix-daemon
'

# Verify nix works
echo "Verifying nix installation..."
incus exec "$BUILD_CONTAINER" -- bash -lc '
    source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
    nix --version
'

# Stop and publish as image
echo "Publishing image..."
incus stop "$BUILD_CONTAINER"

# Remove old image if exists
incus image delete "$IMAGE_NAME" 2>/dev/null || true

incus publish "$BUILD_CONTAINER" --alias "$IMAGE_NAME" --reuse

# Clean up build container
incus delete "$BUILD_CONTAINER"

echo "Base image '$IMAGE_NAME' created successfully"
