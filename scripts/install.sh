#!/usr/bin/env bash
set -euo pipefail
trap 'echo "Espejismo installer failed near line ${LINENO}. Re-run with: curl -fsSL https://raw.githubusercontent.com/tianrking/Espejismo/main/scripts/install.sh -o /tmp/espejismo-install.sh && bash -x /tmp/espejismo-install.sh" >&2' ERR

REPO="${ESPEJISMO_REPO:-tianrking/Espejismo}"
VERSION="${ESPEJISMO_VERSION:-latest}"
ARCHIVE_URL="${ESPEJISMO_ARCHIVE_URL:-}"
ROLE="${ESPEJISMO_ROLE:-}"
INSTALL_DIR="${ESPEJISMO_INSTALL_DIR:-}"
CONFIG_DIR="${ESPEJISMO_CONFIG_DIR:-}"
SERVICE_NAME="${ESPEJISMO_SERVICE_NAME:-espejismo}"
START_NOW="${ESPEJISMO_START:-1}"
INSTALL_TMPDIR=""
ADMIN_TOKEN="${ESPEJISMO_ADMIN_TOKEN:-}"
PSK="${ESPEJISMO_PSK:-}"
SERVER="${ESPEJISMO_SERVER:-}"
LISTEN="${ESPEJISMO_LISTEN:-0.0.0.0:6690}"
PUBLIC_ENDPOINT="${ESPEJISMO_PUBLIC_ENDPOINT:-}"
PUBLIC_HOST="${ESPEJISMO_PUBLIC_HOST:-}"
SOCKS5_LISTEN="${ESPEJISMO_SOCKS5_LISTEN:-127.0.0.1:6680}"
HTTP_LISTEN="${ESPEJISMO_HTTP_LISTEN:-127.0.0.1:6681}"
ADMIN_LISTEN="${ESPEJISMO_ADMIN_LISTEN:-}"
LOCAL_USER="${ESPEJISMO_LOCAL_AUTH_USER:-local-user}"
LOCAL_PASSWORD="${ESPEJISMO_LOCAL_AUTH_PASSWORD:-}"
PROFILE="${ESPEJISMO_PROFILE:-balanced}"

is_tty() {
  [[ -t 0 && -t 1 ]]
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

random_secret() {
  if command -v openssl >/dev/null 2>&1; then
    openssl rand -base64 32
  elif command -v python3 >/dev/null 2>&1; then
    python3 - <<'PY'
import base64
import os

print(base64.b64encode(os.urandom(32)).decode())
PY
  elif command -v base64 >/dev/null 2>&1; then
    dd if=/dev/urandom bs=32 count=1 2>/dev/null | base64 | tr -d '\n'
    echo
  else
    echo "cannot generate a random secret: install openssl or python3, or set ESPEJISMO_PSK" >&2
    exit 1
  fi
}

prompt_default() {
  local var_name="$1"
  local label="$2"
  local default="$3"
  local secret="${4:-0}"
  local current="${!var_name:-}"
  if [[ -n "${current}" ]] || ! is_tty; then
    printf -v "${var_name}" '%s' "${current:-$default}"
    return
  fi
  local value
  if [[ "${secret}" == "1" ]]; then
    read -r -s -p "${label} [auto-random]: " value
    echo
  else
    read -r -p "${label} [${default}]: " value
  fi
  printf -v "${var_name}" '%s' "${value:-$default}"
}

select_role() {
  if [[ -n "${ROLE}" ]]; then
    return
  fi
  if is_tty; then
    echo "Choose install mode:"
    echo "  1) local  - run SOCKS5/HTTP client on this machine"
    echo "  2) remote - run server endpoint on this machine"
    read -r -p "Mode [local]: " choice
    case "${choice:-local}" in
      2|remote|server) ROLE="remote" ;;
      *) ROLE="local" ;;
    esac
  else
    if [[ "${EUID}" -eq 0 && "$(uname -s)" == "Linux" ]]; then
      ROLE="remote"
    else
      ROLE="local"
    fi
  fi
}

detect_package() {
  local os arch
  case "$(uname -s)" in
    Linux) os="linux" ;;
    Darwin) os="darwin" ;;
    *) echo "unsupported OS: $(uname -s)" >&2; exit 1 ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) arch="amd64" ;;
    i386|i486|i586|i686) arch="386" ;;
    aarch64|arm64) arch="arm64" ;;
    armv7l|armv7*) arch="armv7" ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
  esac
  if [[ "${os}" == "darwin" && "${arch}" != "arm64" ]]; then
    echo "darwin-amd64 release artifact is not currently published" >&2
    exit 1
  fi
  printf 'espejismo-%s-%s' "${os}" "${arch}"
}

download_archive() {
  local dest="$1"
  local pkg="$2"
  if [[ -n "${ARCHIVE_URL}" ]]; then
    curl -fsSL "${ARCHIVE_URL}" -o "${dest}"
    return
  fi
  if [[ "${VERSION}" == "latest" ]]; then
    curl -fsSL "https://github.com/${REPO}/releases/latest/download/${pkg}.tar.gz" -o "${dest}"
  else
    curl -fsSL "https://github.com/${REPO}/releases/download/${VERSION}/${pkg}.tar.gz" -o "${dest}"
  fi
}

toml_escape() {
  printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

public_endpoint() {
  if [[ -n "${PUBLIC_ENDPOINT}" ]]; then
    validate_public_endpoint "${PUBLIC_ENDPOINT}"
    printf '%s' "${PUBLIC_ENDPOINT}"
    return
  fi
  local host port endpoint
  port="$(listen_port "${LISTEN}")"
  host="$(detect_public_host)"
  endpoint="$(format_endpoint "${host}" "${port}")"
  validate_public_endpoint "${endpoint}"
  printf '%s' "${endpoint}"
}

listen_port() {
  local value="$1"
  printf '%s' "${value##*:}"
}

format_endpoint() {
  local host="$1"
  local port="$2"
  if [[ "${host}" == *:* && "${host}" != \[*\] ]]; then
    printf '[%s]:%s' "${host}" "${port}"
  else
    printf '%s:%s' "${host}" "${port}"
  fi
}

endpoint_host() {
  local endpoint="$1"
  if [[ "${endpoint}" == \[*\]:* ]]; then
    endpoint="${endpoint#\[}"
    printf '%s' "${endpoint%%\]*}"
  else
    printf '%s' "${endpoint%:*}"
  fi
}

detect_public_host() {
  if [[ -n "${PUBLIC_HOST}" ]]; then
    printf '%s' "${PUBLIC_HOST}"
    return
  fi

  local url candidate
  for url in \
    "https://api.ipify.org" \
    "https://ifconfig.me/ip" \
    "https://checkip.amazonaws.com"; do
    candidate="$(curl -fsSL --max-time 4 "${url}" 2>/dev/null | tr -d '[:space:]' || true)"
    if [[ "${candidate}" =~ ^[0-9A-Fa-f:.]+$ && "${candidate}" != "0.0.0.0" && "${candidate}" != "::" ]]; then
      printf '%s' "${candidate}"
      return
    fi
  done

  candidate="$(hostname -I 2>/dev/null | awk '{print $1}')"
  if [[ -n "${candidate}" ]]; then
    echo "WARN could not detect public IP from external services; falling back to local address ${candidate}" >&2
    printf '%s' "${candidate}"
    return
  fi

  echo "cannot determine public endpoint; set ESPEJISMO_PUBLIC_ENDPOINT=host:port or ESPEJISMO_PUBLIC_HOST=host" >&2
  exit 1
}

validate_public_endpoint() {
  local endpoint="$1"
  local host port
  host="$(endpoint_host "${endpoint}")"
  port="${endpoint##*:}"
  if [[ -z "${host}" || "${host}" == "0.0.0.0" || "${host}" == "::" || "${host}" == "*" ]]; then
    echo "invalid ESPEJISMO_PUBLIC_ENDPOINT '${endpoint}': use a client-reachable public IP or domain, not a listen address" >&2
    exit 1
  fi
  if [[ "${port}" == "${endpoint}" || ! "${port}" =~ ^[0-9]+$ || "${port}" -lt 1 || "${port}" -gt 65535 ]]; then
    echo "invalid ESPEJISMO_PUBLIC_ENDPOINT '${endpoint}': expected host:port, for example proxy.example.com:6690" >&2
    exit 1
  fi
  case "${host}" in
    127.*|localhost|10.*|192.168.*|172.1[6-9].*|172.2[0-9].*|172.3[0-1].*)
      echo "WARN public endpoint '${endpoint}' looks private/local; this is fine for local tests but remote clients may not reach it" >&2
      ;;
  esac
}

write_config() {
  local config="$1"
  local admin_default
  if [[ "${ROLE}" == "remote" ]]; then
    admin_default="127.0.0.1:9090"
  else
    admin_default="127.0.0.1:9091"
  fi
  ADMIN_LISTEN="${ADMIN_LISTEN:-$admin_default}"
  cat >"${config}" <<EOF
[shared]
psk = "$(toml_escape "${PSK}")"
clock_skew_secs = 30
puzzle_bits = 12
max_padding = 64
jitter_ms = 0
padding_chance_percent = 35
tunnel_buffer = 1048576
idle_timeout_secs = 300
max_streams = 256
max_physical_connections = 1024
key_update_frames = 16384

[shared.tcp]
nodelay = true
keepalive_secs = 30
heartbeat_secs = 30
user_timeout_ms = 30000
send_buffer_bytes = 1048576
recv_buffer_bytes = 1048576

[shared.mux]
mode = "yamux"
native_initial_window_bytes = 1048576
native_stream_buffer_frames = 128
native_send_queue_frames = 64
native_idle_timeout_secs = 300
native_drain_timeout_secs = 30

[shared.pacing]
enabled = true
max_bytes_per_sec = 0
burst_bytes = 65536
min_write_bytes = 1024

[shared.obfuscation]
profile = "$(toml_escape "${PROFILE}")"
chunk_policy = "$(toml_escape "${PROFILE}")"
randomize_chunks = true
min_chunk = 4096
max_chunk = 16384

[shared.stealth]
frame_size = 4096
tick_ms = 50

[local]
server = "$(toml_escape "${SERVER}")"
socks5_listen = "${SOCKS5_LISTEN}"
http_listen = "${HTTP_LISTEN}"
handshake_padding = 256

[local.auth]
username = "$(toml_escape "${LOCAL_USER}")"
password = "$(toml_escape "${LOCAL_PASSWORD}")"

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3
max_connection_age_secs = 3600

[logging]
level = "info"
format = "compact"
ansi = true
file = "${CONFIG_DIR}/espejismo-${ROLE}.log"

[admin]
listen = "${ADMIN_LISTEN}"
token = "$(toml_escape "${ADMIN_TOKEN}")"

[remote]
listen = "${LISTEN}"
handshake_timeout_ms = 3000
reject_delay_ms = 0
max_handshake_padding = 1024
replay_window_secs = 60
cold_start_delay_ms = 35
tarpit_max = 1024
tarpit_hold_secs = 300

[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]
block_ports = [25]
block_hosts = ["169.254.169.254", "metadata.google.internal"]
EOF
}

write_manager() {
  local manager="$1"
  local bin_dir="$2"
  local config="$3"
  local log_file="${CONFIG_DIR}/espejismo-${ROLE}.log"
  local pid_file="${CONFIG_DIR}/espejismo-${ROLE}.pid"
  local binary="espejismo-local"
  [[ "${ROLE}" == "remote" ]] && binary="espejismo-remote"
  cat >"${manager}" <<EOF
#!/usr/bin/env bash
set -euo pipefail
ROLE="${ROLE}"
BIN="${bin_dir}/${binary}"
CONFIG="${config}"
PID_FILE="${pid_file}"
LOG_FILE="${log_file}"
ADMIN="http://${ADMIN_LISTEN}"
TOKEN="$(toml_escape "${ADMIN_TOKEN}")"
SERVICE="${SERVICE_NAME}-${ROLE}.service"
SERVER_ENDPOINT="$(toml_escape "${SERVER}")"
SOCKS5_ADDR="$(toml_escape "${SOCKS5_LISTEN}")"
HTTP_ADDR="$(toml_escape "${HTTP_LISTEN}")"
LOCAL_AUTH_USER="$(toml_escape "${LOCAL_USER}")"
LOCAL_AUTH_PASSWORD="$(toml_escape "${LOCAL_PASSWORD}")"

has_systemd_service() {
  command -v systemctl >/dev/null 2>&1 && systemctl list-unit-files "\${SERVICE}" >/dev/null 2>&1
}

cmd_start() {
  if has_systemd_service; then
    sudo systemctl start "\${SERVICE}"
    return
  fi
  if [[ -f "\${PID_FILE}" ]] && kill -0 "\$(cat "\${PID_FILE}")" 2>/dev/null; then
    echo "\${ROLE} already running: \$(cat "\${PID_FILE}")"
    return
  fi
  nohup "\${BIN}" --config "\${CONFIG}" >>"\${LOG_FILE}" 2>&1 &
  echo \$! >"\${PID_FILE}"
  echo "started \${ROLE}: \$(cat "\${PID_FILE}")"
}

cmd_stop() {
  if has_systemd_service; then
    sudo systemctl stop "\${SERVICE}"
    return
  fi
  if [[ -f "\${PID_FILE}" ]]; then
    kill "\$(cat "\${PID_FILE}")" 2>/dev/null || true
    rm -f "\${PID_FILE}"
  fi
  echo "stopped \${ROLE}"
}

cmd_status() {
  if has_systemd_service; then
    systemctl status "\${SERVICE}" --no-pager || true
  elif [[ -f "\${PID_FILE}" ]] && kill -0 "\$(cat "\${PID_FILE}")" 2>/dev/null; then
    echo "\${ROLE} running: \$(cat "\${PID_FILE}")"
  else
    echo "\${ROLE} stopped"
  fi
  if command -v curl >/dev/null 2>&1; then
    curl -fsS -H "Authorization: Bearer \${TOKEN}" "\${ADMIN}/status" 2>/dev/null || true
    echo
  fi
}

cmd_restart() {
  cmd_stop
  sleep 1
  cmd_start
}

cmd_reload() {
  curl -fsS -X POST -H "Authorization: Bearer \${TOKEN}" "\${ADMIN}/reload"
  echo
}

cmd_logs() {
  if has_systemd_service; then
    journalctl -u "\${SERVICE}" -f
  else
    touch "\${LOG_FILE}"
    tail -f "\${LOG_FILE}"
  fi
}

cmd_edit() {
  "\${EDITOR:-vi}" "\${CONFIG}"
}

cmd_profile() {
  if [[ "\${ROLE}" != "remote" ]]; then
    echo "profile export is most useful on a remote install" >&2
  fi
  "${bin_dir}/espejismo-local" --config "\${CONFIG}" --print-client-profile --profile-name default
}

shell_quote() {
  printf '%q' "\$1"
}

cmd_connect() {
  if [[ "\${ROLE}" == "local" ]]; then
    local auth socks http
    auth="\$(shell_quote "\${LOCAL_AUTH_USER}:\${LOCAL_AUTH_PASSWORD}")"
    socks="\$(shell_quote "\${SOCKS5_ADDR}")"
    http="\$(shell_quote "http://\${HTTP_ADDR}")"
    echo "Local proxy is ready."
    echo "  SOCKS5: \${SOCKS5_ADDR}"
    echo "  HTTP:   \${HTTP_ADDR}"
    echo "  User:   \${LOCAL_AUTH_USER}"
    echo "  Pass:   \${LOCAL_AUTH_PASSWORD}"
    echo
    echo "Test commands:"
    echo "  curl --proxy-user \${auth} --socks5-hostname \${socks} https://ifconfig.me"
    echo "  curl --proxy-user \${auth} -x \${http} https://ifconfig.me"
    echo
    echo "Browser/app settings:"
    echo "  SOCKS5 host/port: \${SOCKS5_ADDR}"
    echo "  HTTP proxy:       \${HTTP_ADDR}"
    echo "  Proxy auth:       \${LOCAL_AUTH_USER} / \${LOCAL_AUTH_PASSWORD}"
    return
  fi

  local profile_url
  profile_url="\$(cmd_profile)"
  echo "Remote endpoint is ready."
  echo "  Public endpoint: \${SERVER_ENDPOINT}"
  echo
  echo "Client import profile:"
  echo "  \${profile_url}"
  echo
  echo "Client one-line start:"
  echo "  espejismo-local --import-profile '\${profile_url}'"
}

case "\${1:-status}" in
  start) cmd_start ;;
  stop) cmd_stop ;;
  restart) cmd_restart ;;
  status) cmd_status ;;
  reload) cmd_reload ;;
  logs) cmd_logs ;;
  edit) cmd_edit ;;
  profile) cmd_profile ;;
  connect) cmd_connect ;;
  config) echo "\${CONFIG}" ;;
  *) echo "usage: \$0 {start|stop|restart|status|reload|logs|edit|profile|connect|config}" >&2; exit 2 ;;
esac
EOF
  chmod 0755 "${manager}"
}

write_systemd_service() {
  local bin_dir="$1"
  local config="$2"
  local manager="$3"
  local binary="espejismo-remote"
  [[ "${ROLE}" == "local" ]] && binary="espejismo-local"
  if [[ "$(uname -s)" != "Linux" || "${EUID}" -ne 0 || ! -d /etc/systemd/system ]]; then
    return
  fi
  cat >"/etc/systemd/system/${SERVICE_NAME}-${ROLE}.service" <<EOF
[Unit]
Description=Espejismo ${ROLE}
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=${bin_dir}/${binary} --config ${config}
Restart=on-failure
RestartSec=3
NoNewPrivileges=true
ReadWritePaths=${CONFIG_DIR}

[Install]
WantedBy=multi-user.target
EOF
  systemctl daemon-reload
  systemctl enable "${SERVICE_NAME}-${ROLE}.service" >/dev/null
  ln -sf "${manager}" "/usr/local/bin/espejismoctl-${ROLE}" 2>/dev/null || true
}

main() {
  echo "Espejismo installer starting..."
  need_cmd curl
  need_cmd tar
  select_role

  if [[ "${ROLE}" != "local" && "${ROLE}" != "remote" ]]; then
    echo "ESPEJISMO_ROLE must be local or remote" >&2
    exit 1
  fi
  echo "Selected install mode: ${ROLE}"
  if ! is_tty; then
    echo "  Override with: ESPEJISMO_ROLE=local|remote bash install.sh"
  fi

  [[ -z "${PSK}" ]] && PSK="$(random_secret)"
  [[ -z "${ADMIN_TOKEN}" ]] && ADMIN_TOKEN="$(random_secret)"
  [[ -z "${LOCAL_PASSWORD}" ]] && LOCAL_PASSWORD="$(random_secret)"

  if [[ "${ROLE}" == "remote" ]]; then
    prompt_default LISTEN "Remote listen address" "${LISTEN}"
    detected_endpoint="$(public_endpoint)"
    prompt_default PUBLIC_ENDPOINT "Public client endpoint" "${detected_endpoint}"
    validate_public_endpoint "${PUBLIC_ENDPOINT}"
    SERVER="${PUBLIC_ENDPOINT}"
    echo "Public client endpoint: ${SERVER}"
  else
    prompt_default SERVER "Remote server endpoint host:port" "${SERVER:-127.0.0.1:6690}"
    prompt_default SOCKS5_LISTEN "Local SOCKS5 listen" "${SOCKS5_LISTEN}"
    prompt_default HTTP_LISTEN "Local HTTP proxy listen" "${HTTP_LISTEN}"
  fi
  prompt_default PSK "PSK" "${PSK}" 1

  if [[ -z "${INSTALL_DIR}" ]]; then
    if [[ "${EUID}" -eq 0 && "${ROLE}" == "remote" ]]; then
      INSTALL_DIR="/opt/espejismo"
    else
      INSTALL_DIR="${HOME}/.espejismo"
    fi
  fi
  CONFIG_DIR="${CONFIG_DIR:-${INSTALL_DIR}/config}"
  local bin_dir="${INSTALL_DIR}/bin"
  local manager="${INSTALL_DIR}/espejismoctl"
  local config="${CONFIG_DIR}/espejismo.toml"
  local archive pkg pkgdir
  INSTALL_TMPDIR="$(mktemp -d)"
  trap 'rm -rf "${INSTALL_TMPDIR}"' EXIT
  archive="${INSTALL_TMPDIR}/espejismo.tar.gz"
  pkg="$(detect_package)"

  echo "Downloading ${pkg} from ${REPO} (${VERSION})..."
  download_archive "${archive}" "${pkg}"
  tar -xzf "${archive}" -C "${INSTALL_TMPDIR}"
  pkgdir="$(find "${INSTALL_TMPDIR}" -maxdepth 1 -type d -name 'espejismo-*' | head -n 1)"
  [[ -n "${pkgdir}" ]] || { echo "invalid release archive" >&2; exit 1; }

  mkdir -p "${bin_dir}" "${CONFIG_DIR}"
  install -m 0755 "${pkgdir}/bin/espejismo-local" "${bin_dir}/espejismo-local"
  install -m 0755 "${pkgdir}/bin/espejismo-remote" "${bin_dir}/espejismo-remote"
  write_config "${config}"
  chmod 0600 "${config}"
  write_manager "${manager}" "${bin_dir}" "${config}"
  write_systemd_service "${bin_dir}" "${config}" "${manager}"

  if [[ "${START_NOW}" == "1" ]]; then
    "${manager}" restart
  fi

  echo
  echo "Espejismo ${ROLE} installed."
  echo "  Install dir: ${INSTALL_DIR}"
  echo "  Config:      ${config}"
  echo "  Manager:     ${manager}"
  echo
  echo "Management:"
  echo "  ${manager} status"
  echo "  ${manager} logs"
  echo "  ${manager} edit"
  echo "  ${manager} reload"
  echo "  ${manager} restart"
  echo "  ${manager} connect"
  echo
  echo "Connection:"
  "${manager}" connect
}

main "$@"
