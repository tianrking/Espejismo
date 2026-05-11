#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PSK="change-me-long-random-secret"
MUX_MODE="${MUX_MODE:-yamux}"
HTTP_ADDR="127.0.0.1"
PORT_BASE=$((20000 + ($$ % 20000)))
HTTP_PORT="${PORT_BASE}"
REMOTE_ADDR="127.0.0.1:$((PORT_BASE + 1))"
REMOTE_DOMAIN_ADDR="localhost:$((PORT_BASE + 1))"
SOCKS5_ADDR="127.0.0.1:$((PORT_BASE + 2))"
HTTP_PROXY_ADDR="127.0.0.1:$((PORT_BASE + 3))"
LOCAL_ADMIN_ADDR="127.0.0.1:$((PORT_BASE + 4))"
REMOTE_ADMIN_ADDR="127.0.0.1:$((PORT_BASE + 5))"
UDP_PORT="$((PORT_BASE + 6))"
CONFIG_FILE="$(mktemp /tmp/espejismo-config.XXXXXX.toml)"
PROBE_TOKEN="probe-$(date +%s)-$$"
POST_BODY="body-${PROBE_TOKEN}"
PROXY_USER="probe-user"
PROXY_PASS="probe-pass"
ADMIN_TOKEN="admin-${PROBE_TOKEN}"

cleanup() {
  if [[ -n "${LOCAL_PID:-}" ]]; then kill "${LOCAL_PID}" 2>/dev/null || true; fi
  if [[ -n "${REMOTE_PID:-}" ]]; then kill "${REMOTE_PID}" 2>/dev/null || true; fi
  if [[ -n "${HTTP_PID:-}" ]]; then kill "${HTTP_PID}" 2>/dev/null || true; fi
  if [[ -n "${UDP_PID:-}" ]]; then kill "${UDP_PID}" 2>/dev/null || true; fi
  rm -f "${CONFIG_FILE}"
}
trap cleanup EXIT

wait_for_port() {
  local host="$1"
  local port="$2"
  local name="$3"
  for _ in {1..80}; do
    if (echo >"/dev/tcp/${host}/${port}") >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "timed out waiting for ${name} at ${host}:${port}" >&2
  return 1
}

cd "${ROOT}"

python3 "${ROOT}/scripts/probe_http_server.py" --host "${HTTP_ADDR}" --port "${HTTP_PORT}" >/tmp/espejismo-http.log 2>&1 &
HTTP_PID=$!
wait_for_port "${HTTP_ADDR}" "${HTTP_PORT}" "http fixture"

cargo run --quiet --bin espejismo-local -- \
  --check-update \
  --update-url "http://${HTTP_ADDR}:${HTTP_PORT}/release/latest" \
  | grep -q "update available: 0.0.5 -> v99.0.0"

cargo run --quiet --bin espejismo-remote -- \
  --check-update \
  --update-url "http://${HTTP_ADDR}:${HTTP_PORT}/release/latest" \
  | grep -q "update available: 0.0.5 -> v99.0.0"

python3 "${ROOT}/scripts/probe_udp_server.py" --host "${HTTP_ADDR}" --port "${UDP_PORT}" >/tmp/espejismo-udp.log 2>&1 &
UDP_PID=$!

cat >"${CONFIG_FILE}" <<EOF
[shared]
psk = "${PSK}"
puzzle_bits = 12
max_streams = 2

[shared.tcp]
nodelay = true
keepalive_secs = 30
heartbeat_secs = 5
user_timeout_ms = 30000
send_buffer_bytes = 1048576
recv_buffer_bytes = 1048576

[shared.mux]
mode = "${MUX_MODE}"
native_initial_window_bytes = 1048576
native_stream_buffer_frames = 128
native_send_queue_frames = 64
native_idle_timeout_secs = 60
native_drain_timeout_secs = 10

[shared.pacing]
enabled = true
max_bytes_per_sec = 0
burst_bytes = 65536
min_write_bytes = 1024

[shared.obfuscation]
profile = "balanced"
chunk_policy = "balanced"
randomize_chunks = true

[local]
server = "${REMOTE_DOMAIN_ADDR}"
socks5_listen = "${SOCKS5_ADDR}"
http_listen = "${HTTP_PROXY_ADDR}"
handshake_padding = 256

[local.auth]
username = "${PROXY_USER}"
password = "${PROXY_PASS}"

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3

[logging]
level = "debug"
format = "json"
ansi = false

[admin]
listen = "${LOCAL_ADMIN_ADDR}"
token = "${ADMIN_TOKEN}"

[remote]
listen = "${REMOTE_ADDR}"
handshake_timeout_ms = 1000
reject_delay_ms = 25
cold_start_delay_ms = 20

[[remote.users]]
name = "smoke"
psk = "${PSK}"

[remote.users.quota]
bytes = 10485760
window_secs = 60

[remote.users.bandwidth]
bytes_per_sec = 10485760

[remote.egress]
allow_ports = [${HTTP_PORT}, ${UDP_PORT}]
EOF

CONFIG_B64="$(base64 <"${CONFIG_FILE}" | tr -d '\n')"
LOCAL_CONFIG_B64="$(cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}" --print-config-base64)"
cargo run --quiet --bin espejismo-local -- --decode-config-base64 "${LOCAL_CONFIG_B64}" \
  | grep -q 'socks5_listen'
REMOTE_CONFIG_B64="$(cargo run --quiet --bin espejismo-remote -- --config "${CONFIG_FILE}" --print-config-base64)"
cargo run --quiet --bin espejismo-remote -- --decode-config-base64 "${REMOTE_CONFIG_B64}" \
  | grep -q 'handshake_timeout_ms'
PROFILE_URL="$(cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}" --print-client-profile --profile-name smoke)"
case "${PROFILE_URL}" in
  espejismo://import/*) ;;
  *) echo "unexpected profile URL: ${PROFILE_URL}" >&2; exit 1 ;;
esac
cargo run --quiet --bin espejismo-local -- --import-profile "${PROFILE_URL}" --print-client-profile --profile-name smoke-imported \
  | grep -q "espejismo://import/"

cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}" --check-config \
  | grep -q "config check passed"
cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}" --tun-enabled --check-config \
  | grep -q "TUN ingress requested"
case "$(uname -s)" in
  Linux)
    cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}" --tun-enabled --tun-auto-route --tun-auto-dns --check-config \
      | grep -q "Linux TUN auto-route requested"
    ;;
  Darwin)
    cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}" --tun-enabled --tun-auto-route --tun-auto-dns --check-config \
      | grep -q "macOS TUN auto-route requested"
    ;;
esac
cargo run --quiet --bin espejismo-remote -- --config "${CONFIG_FILE}" --check-config \
  | grep -q "config check passed"

cargo run --quiet --bin espejismo-remote -- \
  --config-base64 "${CONFIG_B64}" \
  --admin-listen "${REMOTE_ADMIN_ADDR}" >/tmp/espejismo-remote.log 2>&1 &
REMOTE_PID=$!
wait_for_port "127.0.0.1" "$((PORT_BASE + 1))" "espejismo remote"
wait_for_port "127.0.0.1" "$((PORT_BASE + 5))" "espejismo remote admin"

cargo run --quiet --bin espejismo-local -- \
  --config "${CONFIG_FILE}" >/tmp/espejismo-local.log 2>&1 &
LOCAL_PID=$!
wait_for_port "127.0.0.1" "$((PORT_BASE + 2))" "SOCKS5 proxy"
wait_for_port "127.0.0.1" "$((PORT_BASE + 3))" "HTTP proxy"
wait_for_port "127.0.0.1" "$((PORT_BASE + 4))" "espejismo local admin"

curl --silent --show-error --max-time 10 \
  --proxy-user "${PROXY_USER}:${PROXY_PASS}" \
  --socks5-hostname "${SOCKS5_ADDR}" \
  -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/socks5/${PROBE_TOKEN}" \
  | grep -q "\"probe\": \"${PROBE_TOKEN}\""

for idx in 1 2 3 4; do
  curl --silent --show-error --max-time 10 \
    --proxy-user "${PROXY_USER}:${PROXY_PASS}" \
    --socks5-hostname "${SOCKS5_ADDR}" \
    -H "X-Espejismo-Probe: ${PROBE_TOKEN}-seq-${idx}" \
    "http://${HTTP_ADDR}:${HTTP_PORT}/probe/sequential/${idx}/${PROBE_TOKEN}" \
    | grep -q "\"path\": \"/probe/sequential/${idx}/${PROBE_TOKEN}\""
done

curl --silent --show-error --max-time 10 \
  --proxy-user "${PROXY_USER}:${PROXY_PASS}" \
  --socks5-hostname "${SOCKS5_ADDR}" \
  -X POST \
  -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
  --data "${POST_BODY}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/post/${PROBE_TOKEN}" \
  | grep -q "\"body\": \"${POST_BODY}\""

curl --silent --show-error --max-time 10 \
  --proxy "http://${HTTP_PROXY_ADDR}" \
  --proxy-user "${PROXY_USER}:${PROXY_PASS}" \
  -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/http/${PROBE_TOKEN}" \
  | grep -q "\"path\": \"/probe/http/${PROBE_TOKEN}\""

curl --silent --show-error --max-time 10 \
  --proxytunnel \
  --proxy "http://${HTTP_PROXY_ADDR}" \
  --proxy-user "${PROXY_USER}:${PROXY_PASS}" \
  -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/connect/${PROBE_TOKEN}" \
  | grep -q "\"path\": \"/probe/connect/${PROBE_TOKEN}\""

python3 "${ROOT}/scripts/probe_socks5_udp.py" \
  --socks-port "$((PORT_BASE + 2))" \
  --username "${PROXY_USER}" \
  --password "${PROXY_PASS}" \
  --target-port "${UDP_PORT}" \
  --payload "${PROBE_TOKEN}" \
  | grep -q "udp-echo:${PROBE_TOKEN}"

HTTP_AUTH_STATUS="$(curl --silent --output /dev/null --write-out "%{http_code}" --max-time 5 \
  --proxy "http://${HTTP_PROXY_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/reject/${PROBE_TOKEN}" || true)"
test "${HTTP_AUTH_STATUS}" = "407"

curl --silent --show-error --max-time 5 \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  "http://${LOCAL_ADMIN_ADDR}/healthz" \
  | grep -q "ok"

curl --silent --show-error --max-time 5 \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  "http://${LOCAL_ADMIN_ADDR}/status" \
  | grep -q "\"tunnel_state\""

curl --silent --show-error --max-time 5 \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  "http://${LOCAL_ADMIN_ADDR}/connections" \
  | grep -q "\"active_physical_connections\""

LOCAL_APPLY_FILE="$(mktemp /tmp/espejismo-local-apply.XXXXXX.toml)"
cp "${CONFIG_FILE}" "${LOCAL_APPLY_FILE}"
python3 - "${LOCAL_APPLY_FILE}" "${PROXY_PASS}" "${PROXY_PASS}-reloaded" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
text = path.read_text()
text = text.replace(f'password = "{sys.argv[2]}"', f'password = "{sys.argv[3]}"')
path.write_text(text)
PY

curl --silent --show-error --max-time 5 \
  -X POST \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  --data-binary "@${LOCAL_APPLY_FILE}" \
  "http://${LOCAL_ADMIN_ADDR}/apply" \
  | grep -q '"applied": true'

curl --silent --show-error --max-time 10 \
  --proxy-user "${PROXY_USER}:${PROXY_PASS}-reloaded" \
  --socks5-hostname "${SOCKS5_ADDR}" \
  -H "X-Espejismo-Probe: ${PROBE_TOKEN}-local-reload" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/local-reload/${PROBE_TOKEN}" \
  | grep -q "\"path\": \"/probe/local-reload/${PROBE_TOKEN}\""

curl --silent --show-error --max-time 5 \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  "http://${REMOTE_ADMIN_ADDR}/metrics" \
  | grep -q "espejismo_stream_opened_total"

curl --silent --show-error --max-time 5 \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  "http://${REMOTE_ADMIN_ADDR}/metrics" \
  | grep -q 'user="smoke"'

python3 - "${CONFIG_FILE}" "${HTTP_PORT}" "${UDP_PORT}" <<'PY'
import sys
from pathlib import Path

path = Path(sys.argv[1])
http_port = sys.argv[2]
udp_port = sys.argv[3]
text = path.read_text()
text = text.replace(
    f"allow_ports = [{http_port}, {udp_port}]",
    f"allow_ports = [{udp_port}]",
)
path.write_text(text)
PY

curl --silent --show-error --max-time 5 \
  -X POST \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  --data-binary "@${CONFIG_FILE}" \
  "http://${REMOTE_ADMIN_ADDR}/apply" \
  | grep -q '"applied": true'

RELOAD_BLOCK_STATUS="$(curl --silent --output /dev/null --write-out "%{http_code}" --max-time 5 \
  --proxy-user "${PROXY_USER}:${PROXY_PASS}" \
  --socks5-hostname "${SOCKS5_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/probe/reload-block/${PROBE_TOKEN}" || true)"
test "${RELOAD_BLOCK_STATUS}" != "200"

ADMIN_AUTH_STATUS="$(curl --silent --output /dev/null --write-out "%{http_code}" --max-time 5 \
  "http://${LOCAL_ADMIN_ADDR}/status" || true)"
test "${ADMIN_AUTH_STATUS}" = "401"

echo "e2e smoke test passed"
