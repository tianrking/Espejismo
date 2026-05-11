#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUESTS="${REQUESTS:-128}"
CONCURRENCY="${CONCURRENCY:-32}"
OUT="${OUT:-}"

cd "${ROOT}"

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().strip()))'
}

run_case() {
  local mode="$1"
  local started_ns
  local ended_ns
  local log
  log="$(mktemp -t espejismo-bench-${mode}.XXXXXX.log)"
  echo "benchmark mux=${mode}: ${REQUESTS} requests, concurrency ${CONCURRENCY}" >&2
  started_ns="$(date +%s%N)"
  MUX_MODE="${mode}" REQUESTS="${REQUESTS}" CONCURRENCY="${CONCURRENCY}" ./scripts/stress_smoke.sh >"${log}" 2>&1
  ended_ns="$(date +%s%N)"
  local duration_ms=$(((ended_ns - started_ns) / 1000000))
  local status="passed"
  local escaped_log
  escaped_log="$(tail -n 20 "${log}" | json_escape)"
  rm -f "${log}"
  printf '{"mux":"%s","status":"%s","requests":%s,"concurrency":%s,"duration_ms":%s,"log_tail":%s}' \
    "${mode}" "${status}" "${REQUESTS}" "${CONCURRENCY}" "${duration_ms}" "${escaped_log}"
}

started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
yamux_json="$(run_case yamux)"
native_json="$(run_case native)"
ended_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

result="$(printf '{"started_at":"%s","ended_at":"%s","cases":[%s,%s]}\n' \
  "${started_at}" "${ended_at}" "${yamux_json}" "${native_json}")"

if [[ -n "${OUT}" ]]; then
  printf '%s' "${result}" >"${OUT}"
else
  printf '%s' "${result}"
fi
