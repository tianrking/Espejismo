#!/usr/bin/env bash
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root, for example: curl -fsSL ... | sudo bash" >&2
  exit 1
fi

ESPEJISMO_REPO="${ESPEJISMO_REPO:-}"
ESPEJISMO_VERSION="${ESPEJISMO_VERSION:-latest}"
ESPEJISMO_ARCHIVE_URL="${ESPEJISMO_ARCHIVE_URL:-}"
ESPEJISMO_LISTEN="${ESPEJISMO_LISTEN:-0.0.0.0:8443}"
ESPEJISMO_ADMIN_LISTEN="${ESPEJISMO_ADMIN_LISTEN:-127.0.0.1:9090}"
ESPEJISMO_ADMIN_TOKEN="${ESPEJISMO_ADMIN_TOKEN:-}"
ESPEJISMO_PSK="${ESPEJISMO_PSK:-}"
ESPEJISMO_CLOCK_SKEW_SECS="${ESPEJISMO_CLOCK_SKEW_SECS:-30}"
ESPEJISMO_PUZZLE_BITS="${ESPEJISMO_PUZZLE_BITS:-12}"
ESPEJISMO_MAX_PADDING="${ESPEJISMO_MAX_PADDING:-64}"
ESPEJISMO_JITTER_MS="${ESPEJISMO_JITTER_MS:-0}"
ESPEJISMO_PADDING_CHANCE_PERCENT="${ESPEJISMO_PADDING_CHANCE_PERCENT:-35}"
ESPEJISMO_BACKPRESSURE_THRESHOLD_MS="${ESPEJISMO_BACKPRESSURE_THRESHOLD_MS:-40}"
ESPEJISMO_BACKPRESSURE_COOLDOWN_MS="${ESPEJISMO_BACKPRESSURE_COOLDOWN_MS:-1000}"
ESPEJISMO_TUNNEL_BUFFER="${ESPEJISMO_TUNNEL_BUFFER:-1048576}"
ESPEJISMO_IDLE_TIMEOUT_SECS="${ESPEJISMO_IDLE_TIMEOUT_SECS:-300}"
ESPEJISMO_MAX_STREAMS="${ESPEJISMO_MAX_STREAMS:-256}"
ESPEJISMO_HANDSHAKE_TIMEOUT_MS="${ESPEJISMO_HANDSHAKE_TIMEOUT_MS:-3000}"
ESPEJISMO_REJECT_DELAY_MS="${ESPEJISMO_REJECT_DELAY_MS:-0}"
ESPEJISMO_MAX_HANDSHAKE_PADDING="${ESPEJISMO_MAX_HANDSHAKE_PADDING:-1024}"
ESPEJISMO_REPLAY_WINDOW_SECS="${ESPEJISMO_REPLAY_WINDOW_SECS:-60}"
ESPEJISMO_COLD_START_DELAY_MS="${ESPEJISMO_COLD_START_DELAY_MS:-35}"
ESPEJISMO_TARPIT_MAX="${ESPEJISMO_TARPIT_MAX:-1024}"
ESPEJISMO_TARPIT_HOLD_SECS="${ESPEJISMO_TARPIT_HOLD_SECS:-300}"
ESPEJISMO_DENY_PRIVATE_IPS="${ESPEJISMO_DENY_PRIVATE_IPS:-true}"
ESPEJISMO_ALLOW_PORTS="${ESPEJISMO_ALLOW_PORTS:-80,443}"
ESPEJISMO_BLOCK_PORTS="${ESPEJISMO_BLOCK_PORTS:-25}"
ESPEJISMO_ALLOW_HOSTS="${ESPEJISMO_ALLOW_HOSTS:-}"
ESPEJISMO_BLOCK_HOSTS="${ESPEJISMO_BLOCK_HOSTS:-169.254.169.254,metadata.google.internal}"
ESPEJISMO_OPEN_UFW="${ESPEJISMO_OPEN_UFW:-0}"

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

random_secret() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -base64 32
  else
    tr -dc 'A-Za-z0-9' </dev/urandom | head -c 48
    echo
  fi
}

toml_string_array() {
  local input="$1"
  if [[ -z "${input}" ]]; then
    printf "[]"
    return
  fi
  local output="["
  local first=1
  IFS=',' read -ra values <<<"${input}"
  for value in "${values[@]}"; do
    value="$(echo "${value}" | xargs)"
    [[ -z "${value}" ]] && continue
    if [[ "${first}" -eq 0 ]]; then
      output+=", "
    fi
    output+="\"${value//\"/\\\"}\""
    first=0
  done
  output+="]"
  printf "%s" "${output}"
}

toml_number_array() {
  local input="$1"
  if [[ -z "${input}" ]]; then
    printf "[]"
    return
  fi
  local output="["
  local first=1
  IFS=',' read -ra values <<<"${input}"
  for value in "${values[@]}"; do
    value="$(echo "${value}" | xargs)"
    [[ -z "${value}" ]] && continue
    if [[ ! "${value}" =~ ^[0-9]+$ ]]; then
      echo "invalid numeric list value: ${value}" >&2
      exit 1
    fi
    if [[ "${first}" -eq 0 ]]; then
      output+=", "
    fi
    output+="${value}"
    first=0
  done
  output+="]"
  printf "%s" "${output}"
}

download_archive() {
  local dest="$1"
  local package_arch
  case "$(uname -m)" in
    x86_64|amd64) package_arch="linux-x86_64" ;;
    i386|i486|i586|i686) package_arch="linux-x86" ;;
    aarch64|arm64) package_arch="linux-arm64" ;;
    armv7l|armv7*) package_arch="linux-arm32" ;;
    *)
      echo "unsupported Ubuntu architecture: $(uname -m)" >&2
      exit 1
      ;;
  esac
  if [[ -n "${ESPEJISMO_ARCHIVE_URL}" ]]; then
    curl -fsSL "${ESPEJISMO_ARCHIVE_URL}" -o "${dest}"
    return
  fi
  if [[ -z "${ESPEJISMO_REPO}" ]]; then
    cat >&2 <<'EOF'
Set ESPEJISMO_REPO=owner/repo or ESPEJISMO_ARCHIVE_URL before running this installer.

Example:
  curl -fsSL https://raw.githubusercontent.com/OWNER/REPO/main/scripts/install-ubuntu-remote.sh \
    | sudo ESPEJISMO_REPO=OWNER/REPO ESPEJISMO_VERSION=latest bash
EOF
    exit 1
  fi
  local base="https://github.com/${ESPEJISMO_REPO}/releases"
  if [[ "${ESPEJISMO_VERSION}" == "latest" ]]; then
    curl -fsSL "${base}/latest/download/espejismo-${package_arch}.tar.gz" -o "${dest}"
  else
    curl -fsSL "${base}/download/${ESPEJISMO_VERSION}/espejismo-${package_arch}.tar.gz" -o "${dest}"
  fi
}

need_cmd curl
need_cmd tar
need_cmd systemctl

if [[ -z "${ESPEJISMO_PSK}" ]]; then
  ESPEJISMO_PSK="$(random_secret)"
fi
if [[ -z "${ESPEJISMO_ADMIN_TOKEN}" ]]; then
  ESPEJISMO_ADMIN_TOKEN="$(random_secret)"
fi

tmpdir="$(mktemp -d)"
cleanup() {
  rm -rf "${tmpdir}"
}
trap cleanup EXIT

archive="${tmpdir}/espejismo-release.tar.gz"
download_archive "${archive}"
tar -xzf "${archive}" -C "${tmpdir}"
pkgdir="$(find "${tmpdir}" -maxdepth 1 -type d -name 'espejismo-*' | head -n 1)"
if [[ -z "${pkgdir}" ]]; then
  echo "release archive did not contain an espejismo package directory" >&2
  exit 1
fi

install -d -m 0755 /usr/local/bin /etc/espejismo /var/log/espejismo
install -m 0755 "${pkgdir}/bin/espejismo-remote" /usr/local/bin/espejismo-remote
install -m 0755 "${pkgdir}/bin/espejismo-local" /usr/local/bin/espejismo-local

if ! id espejismo >/dev/null 2>&1; then
  useradd --system --create-home --shell /usr/sbin/nologin espejismo
fi
chown -R espejismo:espejismo /etc/espejismo /var/log/espejismo

allow_hosts="$(toml_string_array "${ESPEJISMO_ALLOW_HOSTS}")"
block_hosts="$(toml_string_array "${ESPEJISMO_BLOCK_HOSTS}")"
allow_ports="$(toml_number_array "${ESPEJISMO_ALLOW_PORTS}")"
block_ports="$(toml_number_array "${ESPEJISMO_BLOCK_PORTS}")"

cat >/etc/espejismo/espejismo.toml <<EOF
[shared]
psk = "${ESPEJISMO_PSK//\"/\\\"}"
clock_skew_secs = ${ESPEJISMO_CLOCK_SKEW_SECS}
puzzle_bits = ${ESPEJISMO_PUZZLE_BITS}
max_padding = ${ESPEJISMO_MAX_PADDING}
jitter_ms = ${ESPEJISMO_JITTER_MS}
padding_chance_percent = ${ESPEJISMO_PADDING_CHANCE_PERCENT}
backpressure_threshold_ms = ${ESPEJISMO_BACKPRESSURE_THRESHOLD_MS}
backpressure_cooldown_ms = ${ESPEJISMO_BACKPRESSURE_COOLDOWN_MS}
tunnel_buffer = ${ESPEJISMO_TUNNEL_BUFFER}
idle_timeout_secs = ${ESPEJISMO_IDLE_TIMEOUT_SECS}
max_streams = ${ESPEJISMO_MAX_STREAMS}

[logging]
level = "info"
format = "json"
ansi = false
file = "/var/log/espejismo/espejismo-remote.log"

[admin]
listen = "${ESPEJISMO_ADMIN_LISTEN}"
token = "${ESPEJISMO_ADMIN_TOKEN//\"/\\\"}"

[remote]
listen = "${ESPEJISMO_LISTEN}"
handshake_timeout_ms = ${ESPEJISMO_HANDSHAKE_TIMEOUT_MS}
reject_delay_ms = ${ESPEJISMO_REJECT_DELAY_MS}
max_handshake_padding = ${ESPEJISMO_MAX_HANDSHAKE_PADDING}
replay_window_secs = ${ESPEJISMO_REPLAY_WINDOW_SECS}
cold_start_delay_ms = ${ESPEJISMO_COLD_START_DELAY_MS}
tarpit_max = ${ESPEJISMO_TARPIT_MAX}
tarpit_hold_secs = ${ESPEJISMO_TARPIT_HOLD_SECS}

[remote.egress]
deny_private_ips = ${ESPEJISMO_DENY_PRIVATE_IPS}
allow_hosts = ${allow_hosts}
block_hosts = ${block_hosts}
allow_ports = ${allow_ports}
block_ports = ${block_ports}
EOF
chmod 0640 /etc/espejismo/espejismo.toml
chown espejismo:espejismo /etc/espejismo/espejismo.toml

cat >/etc/systemd/system/espejismo-remote.service <<'EOF'
[Unit]
Description=Espejismo remote encrypted tunnel endpoint
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=espejismo
Group=espejismo
ExecStart=/usr/local/bin/espejismo-remote --config /etc/espejismo/espejismo.toml
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
PrivateTmp=true
ProtectHome=true
ProtectSystem=strict
ReadOnlyPaths=/etc/espejismo
ReadWritePaths=/var/log/espejismo

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now espejismo-remote.service

if [[ "${ESPEJISMO_OPEN_UFW}" == "1" ]] && command -v ufw >/dev/null 2>&1; then
  listen_port="${ESPEJISMO_LISTEN##*:}"
  ufw allow "${listen_port}/tcp"
fi

server_host="$(hostname -I 2>/dev/null | awk '{print $1}')"
server_host="${server_host:-YOUR_SERVER_IP}"

cat <<EOF
Espejismo remote is installed and running.

Server config:
  /etc/espejismo/espejismo.toml

Status:
  systemctl status espejismo-remote --no-pager
  journalctl -u espejismo-remote -f

Client TOML starter:

[shared]
psk = "${ESPEJISMO_PSK}"

[local]
server = "${server_host}:${ESPEJISMO_LISTEN##*:}"
socks5_listen = "127.0.0.1:1080"
http_listen = "127.0.0.1:8080"

[local.auth]
username = "local-user"
password = "$(random_secret)"

Keep the PSK and profile/config private.
EOF
