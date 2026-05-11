#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUESTS="${REQUESTS:-200}"
CONCURRENCY="${CONCURRENCY:-32}"
SOAK_SECS="${SOAK_SECS:-0}"
PORT_BASE="${PORT_BASE:-$((26000 + ($$ % 20000)))}"
HTTP_ADDR="127.0.0.1"
HTTP_PORT="${PORT_BASE}"
REMOTE_ADDR="127.0.0.1:$((PORT_BASE + 1))"
SOCKS5_ADDR="127.0.0.1:$((PORT_BASE + 2))"
HTTP_PROXY_ADDR="127.0.0.1:$((PORT_BASE + 3))"
CONFIG_FILE="$(mktemp -t espejismo-stress.XXXXXX.toml)"
PROBE_TOKEN="stress-$(date +%s)-$$"
MUX_MODE="${MUX_MODE:-yamux}"
PIDS=()

cleanup() {
  for pid in "${PIDS[@]:-}"; do
    kill "${pid}" 2>/dev/null || true
  done
  rm -f "${CONFIG_FILE}"
}
trap cleanup EXIT

wait_for_port() {
  local host="$1"
  local port="$2"
  local name="$3"
  for _ in $(seq 1 80); do
    if python3 - "$host" "$port" 2>/dev/null <<'PY'
import socket
import sys

host = sys.argv[1]
port = int(sys.argv[2])
with socket.create_connection((host, port), timeout=0.25):
    pass
PY
    then
      return
    fi
    sleep 0.25
  done
  echo "timed out waiting for ${name} at ${host}:${port}" >&2
  exit 1
}

start_process() {
  local name="$1"
  shift
  "$@" >"/tmp/espejismo-${name}-$$.out.log" 2>"/tmp/espejismo-${name}-$$.err.log" &
  PIDS+=("$!")
}

run_parallel_requests() {
  local count="$1"
  local concurrency="$2"
  local mode="$3"
  local request_pids=()
  for idx in $(seq 1 "$count"); do
    (
      path="/stress/${mode}/${idx}/${PROBE_TOKEN}"
      if [[ "$mode" == "socks" || ( "$mode" == "mixed" && $((idx % 2)) -eq 0 ) ]]; then
        curl --silent --show-error --max-time 10 \
          --retry 3 --retry-delay 1 --retry-all-errors \
          --socks5-hostname "${SOCKS5_ADDR}" \
          -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
          "http://${HTTP_ADDR}:${HTTP_PORT}${path}" \
          | grep -q "${PROBE_TOKEN}"
      else
        curl --silent --show-error --max-time 10 \
          --retry 3 --retry-delay 1 --retry-all-errors \
          --proxy "http://${HTTP_PROXY_ADDR}" \
          -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
          "http://${HTTP_ADDR}:${HTTP_PORT}${path}" \
          | grep -q "${PROBE_TOKEN}"
      fi
    ) &
    request_pids+=("$!")
    if [[ "${#request_pids[@]}" -ge "${concurrency}" ]]; then
      for pid in "${request_pids[@]}"; do
        wait "${pid}"
      done
      request_pids=()
    fi
  done
  for pid in "${request_pids[@]}"; do
    wait "${pid}"
  done
}

cd "${ROOT}"

start_process stress-http python3 "${ROOT}/scripts/probe_http_server.py" --host "${HTTP_ADDR}" --port "${HTTP_PORT}"
wait_for_port "${HTTP_ADDR}" "${HTTP_PORT}" "http fixture"

cat >"${CONFIG_FILE}" <<EOF
[shared]
psk = "stress-secret-that-is-long-enough"
puzzle_bits = 8
max_streams = 128
idle_timeout_secs = 60

[shared.obfuscation]
profile = "bulk"
chunk_policy = "bulk"
randomize_chunks = true

[shared.tcp]
nodelay = true
keepalive_secs = 30
heartbeat_secs = 5
send_buffer_bytes = 1048576
recv_buffer_bytes = 1048576

[shared.mux]
mode = "${MUX_MODE}"
native_initial_window_bytes = 1048576
native_stream_buffer_frames = 128
native_idle_timeout_secs = 60

[shared.pacing]
enabled = true
max_bytes_per_sec = 0
burst_bytes = 65536
min_write_bytes = 1024

[local]
server = "${REMOTE_ADDR}"
socks5_listen = "${SOCKS5_ADDR}"
http_listen = "${HTTP_PROXY_ADDR}"
handshake_padding = 256

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3

[logging]
level = "info"
format = "compact"
ansi = false

[remote]
listen = "${REMOTE_ADDR}"
handshake_timeout_ms = 1000
reject_delay_ms = 0
cold_start_delay_ms = 0

[remote.egress]
allow_ports = [${HTTP_PORT}]
EOF

start_process stress-remote cargo run --quiet --bin espejismo-remote -- --config "${CONFIG_FILE}"
wait_for_port "127.0.0.1" "$((PORT_BASE + 1))" "remote"

start_process stress-local cargo run --quiet --bin espejismo-local -- --config "${CONFIG_FILE}"
wait_for_port "127.0.0.1" "$((PORT_BASE + 2))" "SOCKS5 proxy"
wait_for_port "127.0.0.1" "$((PORT_BASE + 3))" "HTTP proxy"

echo "stress: 1 stream download-like request"
curl --silent --show-error --max-time 10 \
  --retry 3 --retry-delay 1 --retry-all-errors \
  --socks5-hostname "${SOCKS5_ADDR}" \
  -H "X-Espejismo-Probe: ${PROBE_TOKEN}" \
  "http://${HTTP_ADDR}:${HTTP_PORT}/bulk/${PROBE_TOKEN}" \
  | grep -q "${PROBE_TOKEN}"

echo "stress: ${REQUESTS} small requests at concurrency ${CONCURRENCY}"
run_parallel_requests "${REQUESTS}" "${CONCURRENCY}" "socks"

echo "stress: mixed large/small lane selection"
run_parallel_requests "${REQUESTS}" "${CONCURRENCY}" "mixed"

echo "stress: remote restart recovery"
kill "${PIDS[1]}" 2>/dev/null || true
sleep 1
start_process stress-remote-restarted cargo run --quiet --bin espejismo-remote -- --config "${CONFIG_FILE}"
wait_for_port "127.0.0.1" "$((PORT_BASE + 1))" "remote restarted"
sleep 1
run_parallel_requests 32 8 "mixed"

if [[ "${SOAK_SECS}" -gt 0 ]]; then
  echo "stress: soak for ${SOAK_SECS}s"
  deadline=$((SECONDS + SOAK_SECS))
  while [[ "${SECONDS}" -lt "${deadline}" ]]; do
    run_parallel_requests 32 8 "mixed"
  done
fi

echo "stress smoke test passed"
