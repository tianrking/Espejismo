#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REQUESTS="${REQUESTS:-128}"
CONCURRENCY="${CONCURRENCY:-32}"

cd "${ROOT}"

run_case() {
  local mode="$1"
  local started
  local ended
  echo "benchmark mux=${mode}: ${REQUESTS} requests, concurrency ${CONCURRENCY}"
  started="$(date +%s)"
  MUX_MODE="${mode}" REQUESTS="${REQUESTS}" CONCURRENCY="${CONCURRENCY}" ./scripts/stress_smoke.sh
  ended="$(date +%s)"
  echo "benchmark mux=${mode}: $((ended - started))s"
}

run_case yamux
run_case native
