#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PSK="change-me-long-random-secret"
HTTP_ADDR="127.0.0.1"
PORT_BASE=$((20000 + ($$ % 20000)))
HTTP_PORT="${PORT_BASE}"
REMOTE_ADDR="127.0.0.1:$((PORT_BASE + 1))"
SOCKS5_ADDR="127.0.0.1:$((PORT_BASE + 2))"
HTTP_PROXY_ADDR="127.0.0.1:$((PORT_BASE + 3))"
LOCAL_ADMIN_ADDR="127.0.0.1:$((PORT_BASE + 4))"
REMOTE_ADMIN_ADDR="127.0.0.1:$((PORT_BASE + 5))"
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

cat >"${CONFIG_FILE}" <<EOF
[shared]
psk = "${PSK}"
puzzle_bits = 12

[local]
server = "${REMOTE_ADDR}"
socks5_listen = "${SOCKS5_ADDR}"
http_listen = "${HTTP_PROXY_ADDR}"
handshake_padding = 256

[local.auth]
username = "${PROXY_USER}"
password = "${PROXY_PASS}"

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

[remote.egress]
allow_ports = [${HTTP_PORT}]
EOF

CONFIG_B64="$(base64 -w0 "${CONFIG_FILE}" 2>/dev/null || base64 "${CONFIG_FILE}" | tr -d '\n')"

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
  | grep -q "\"role\": \"local\""

curl --silent --show-error --max-time 5 \
  -H "Authorization: Bearer ${ADMIN_TOKEN}" \
  "http://${REMOTE_ADMIN_ADDR}/metrics" \
  | grep -q "espejismo_stream_opened_total"

ADMIN_AUTH_STATUS="$(curl --silent --output /dev/null --write-out "%{http_code}" --max-time 5 \
  "http://${LOCAL_ADMIN_ADDR}/status" || true)"
test "${ADMIN_AUTH_STATUS}" = "401"

echo "e2e smoke test passed"
