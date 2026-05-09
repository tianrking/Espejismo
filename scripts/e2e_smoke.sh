#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PSK="change-me-long-random-secret"
HTTP_ADDR="127.0.0.1"
HTTP_PORT="18080"
REMOTE_ADDR="127.0.0.1:18443"
SOCKS5_ADDR="127.0.0.1:11080"
HTTP_PROXY_ADDR="127.0.0.1:18081"
CONFIG_FILE="$(mktemp /tmp/espejismo-config.XXXXXX.toml)"

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

python3 -m http.server "${HTTP_PORT}" --bind "${HTTP_ADDR}" >/tmp/espejismo-http.log 2>&1 &
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

[remote]
listen = "${REMOTE_ADDR}"
handshake_timeout_ms = 1000
reject_delay_ms = 25
cold_start_delay_ms = 20
EOF

CONFIG_B64="$(base64 -w0 "${CONFIG_FILE}" 2>/dev/null || base64 "${CONFIG_FILE}" | tr -d '\n')"

cargo run --quiet --bin espejismo-remote -- \
  --config-base64 "${CONFIG_B64}" >/tmp/espejismo-remote.log 2>&1 &
REMOTE_PID=$!
wait_for_port "127.0.0.1" "18443" "espejismo remote"

cargo run --quiet --bin espejismo-local -- \
  --config "${CONFIG_FILE}" >/tmp/espejismo-local.log 2>&1 &
LOCAL_PID=$!
wait_for_port "127.0.0.1" "11080" "SOCKS5 proxy"
wait_for_port "127.0.0.1" "18081" "HTTP proxy"

curl --silent --show-error --max-time 10 \
  --socks5-hostname "${SOCKS5_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/" | grep -q "Directory listing"

curl --silent --show-error --max-time 10 \
  --socks5-hostname "${SOCKS5_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/README.md" | grep -q "Espejismo"

curl --silent --show-error --max-time 10 \
  --proxy "http://${HTTP_PROXY_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/README.md" | grep -q "Espejismo"

echo "e2e smoke test passed"
