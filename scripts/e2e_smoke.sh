#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PSK="change-me-long-random-secret"
HTTP_ADDR="127.0.0.1"
HTTP_PORT="18080"
REMOTE_ADDR="127.0.0.1:18443"
LOCAL_ADDR="127.0.0.1:11080"

cleanup() {
  if [[ -n "${LOCAL_PID:-}" ]]; then kill "${LOCAL_PID}" 2>/dev/null || true; fi
  if [[ -n "${REMOTE_PID:-}" ]]; then kill "${REMOTE_PID}" 2>/dev/null || true; fi
  if [[ -n "${HTTP_PID:-}" ]]; then kill "${HTTP_PID}" 2>/dev/null || true; fi
}
trap cleanup EXIT

cd "${ROOT}"

python3 -m http.server "${HTTP_PORT}" --bind "${HTTP_ADDR}" >/tmp/espejismo-http.log 2>&1 &
HTTP_PID=$!

ESPEJISMO_PSK="${PSK}" cargo run --quiet --bin espejismo-remote -- \
  --listen "${REMOTE_ADDR}" \
  --puzzle-bits 12 \
  --handshake-timeout-ms 1000 \
  --reject-delay-ms 25 \
  --cold-start-delay-ms 20 >/tmp/espejismo-remote.log 2>&1 &
REMOTE_PID=$!

sleep 1

ESPEJISMO_PSK="${PSK}" cargo run --quiet --bin espejismo-local -- \
  --listen "${LOCAL_ADDR}" \
  --server "${REMOTE_ADDR}" \
  --puzzle-bits 12 \
  --handshake-padding 256 >/tmp/espejismo-local.log 2>&1 &
LOCAL_PID=$!

sleep 1

curl --silent --show-error --max-time 10 \
  --socks5-hostname "${LOCAL_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/" | grep -q "Directory listing"

curl --silent --show-error --max-time 10 \
  --socks5-hostname "${LOCAL_ADDR}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/README.md" | grep -q "Espejismo"

echo "e2e smoke test passed"
