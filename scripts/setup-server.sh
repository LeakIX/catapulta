#!/usr/bin/env bash
# Setup script for freshly provisioned Ubuntu servers.
# Installs Docker, configures the firewall, and starts a
# placeholder Caddy reverse proxy.
#
# Usage: setup-server.sh <domain> <remote_dir>
set -euo pipefail

DOMAIN="${1:?Usage: setup-server.sh <domain> <remote_dir>}"
REMOTE_DIR="${2:?Usage: setup-server.sh <domain> <remote_dir>}"

# Kill unattended-upgrades permanently
echo "Stopping unattended-upgrades..."
systemctl stop unattended-upgrades 2>/dev/null || true
systemctl disable unattended-upgrades 2>/dev/null || true
systemctl mask unattended-upgrades 2>/dev/null || true
pkill -9 unattended-upgr 2>/dev/null || true
pkill -9 apt-get 2>/dev/null || true
pkill -9 dpkg 2>/dev/null || true
sleep 5

# Wait for apt locks
echo "Waiting for apt locks..."
while fuser /var/lib/dpkg/lock-frontend \
    /var/lib/dpkg/lock \
    /var/lib/apt/lists/lock \
    /var/cache/apt/archives/lock \
    >/dev/null 2>&1; do
    echo "  Locks still held, waiting..."
    sleep 3
done
echo "apt locks released"

APT_OPTS="-o DPkg::Lock::Timeout=120"

# Install Docker
if ! command -v docker &>/dev/null; then
    echo "Installing Docker..."
    # shellcheck disable=SC2086
    apt-get $APT_OPTS update
    # shellcheck disable=SC2086
    apt-get $APT_OPTS install -y ca-certificates curl
    install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg \
        -o /etc/apt/keyrings/docker.asc
    chmod a+r /etc/apt/keyrings/docker.asc
    # shellcheck source=/dev/null
    . /etc/os-release
    echo \
        "deb [arch=$(dpkg --print-architecture) \
        signed-by=/etc/apt/keyrings/docker.asc] \
        https://download.docker.com/linux/ubuntu \
        $VERSION_CODENAME \
        stable" > /etc/apt/sources.list.d/docker.list
    # shellcheck disable=SC2086
    apt-get $APT_OPTS update
    # shellcheck disable=SC2086
    apt-get $APT_OPTS install -y \
        docker-ce \
        docker-ce-cli \
        containerd.io \
        docker-compose-plugin
    systemctl enable docker
    systemctl start docker
else
    echo "Docker already installed"
    docker --version
fi

# Setup firewall
ufw allow OpenSSH
ufw allow 80/tcp
ufw allow 443/tcp
ufw --force enable

# Create app directory
mkdir -p "$REMOTE_DIR"

# Write placeholder Caddyfile
cat > "$REMOTE_DIR/Caddyfile" << CADDY
$DOMAIN {
    respond "Service is being deployed..." 503
}
CADDY

# Write minimal docker-compose for Caddy only
cat > "$REMOTE_DIR/docker-compose.yml" << 'COMPOSE'
services:
  caddy:
    image: caddy:2-alpine
    container_name: app-caddy
    restart: unless-stopped
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./Caddyfile:/etc/caddy/Caddyfile:ro
      - caddy-data:/data
      - caddy-config:/config

volumes:
  caddy-data:
    driver: local
  caddy-config:
    driver: local
COMPOSE

# Start Caddy
cd "$REMOTE_DIR"
docker compose pull
docker compose up -d

echo "Setup complete!"
