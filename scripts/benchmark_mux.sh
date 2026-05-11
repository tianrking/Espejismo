#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-}"

cd "${ROOT}"

json_escape() {
  python3 -c 'import json,sys; print(json.dumps(sys.stdin.read().strip()))'
}

run_case() {
  local mode="$1"
  local scenario="$2"
  local requests="$3"
  local concurrency="$4"
  local rotation_secs="$5"
  local log
  local started_ns
  local ended_ns
  log="$(mktemp -t espejismo-bench-${mode}-${scenario}.XXXXXX.log)"
  echo "benchmark mux=${mode} scenario=${scenario} requests=${requests} concurrency=${concurrency}" >&2
  started_ns="$(date +%s%N)"
  MUX_MODE="${mode}" \
    REQUESTS="${requests}" \
    CONCURRENCY="${concurrency}" \
    MAX_CONNECTION_AGE_SECS="${rotation_secs}" \
    ./scripts/stress_smoke.sh >"${log}" 2>&1
  ended_ns="$(date +%s%N)"
  local duration_ms=$(((ended_ns - started_ns) / 1000000))
  local escaped_log
  escaped_log="$(tail -n 20 "${log}" | json_escape)"
  rm -f "${log}"
  printf '{"mux":"%s","scenario":"%s","status":"passed","requests":%s,"concurrency":%s,"max_connection_age_secs":%s,"duration_ms":%s,"log_tail":%s}' \
    "${mode}" "${scenario}" "${requests}" "${concurrency}" "${rotation_secs}" "${duration_ms}" "${escaped_log}"
}

run_suite_for_mode() {
  local mode="$1"
  local one small mixed rotation
  one="$(run_case "${mode}" one_stream 1 1 3600)"
  small="$(run_case "${mode}" small_32 32 32 3600)"
  mixed="$(run_case "${mode}" mixed_bulk_interactive 128 32 3600)"
  rotation="$(run_case "${mode}" session_rotation 64 16 1)"
  printf '%s,%s,%s,%s' "${one}" "${small}" "${mixed}" "${rotation}"
}

started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
yamux_cases="$(run_suite_for_mode yamux)"
native_cases="$(run_suite_for_mode native)"
ended_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

result="$(printf '{"started_at":"%s","ended_at":"%s","cases":[%s,%s]}\n' \
  "${started_at}" "${ended_at}" "${yamux_cases}" "${native_cases}")"

if [[ -n "${OUT}" ]]; then
  printf '%s' "${result}" >"${OUT}"
else
  printf '%s' "${result}"
fi
