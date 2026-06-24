#!/usr/bin/env bash
set -euo pipefail

PROXY_URL="${ESPEJISMO_PROXY_URL:-http://127.0.0.1:16681}"
DIRECT_DOWNLOAD_URL="${ESPEJISMO_DIRECT_DOWNLOAD_URL:-http://127.0.0.1:18082/256m.bin}"
PROXY_DOWNLOAD_URL="${ESPEJISMO_PROXY_DOWNLOAD_URL:-http://127.0.0.1:18082/256m.bin}"
DIRECT_UPLOAD_URL="${ESPEJISMO_DIRECT_UPLOAD_URL:-http://127.0.0.1:18082/upload}"
PROXY_UPLOAD_URL="${ESPEJISMO_PROXY_UPLOAD_URL:-http://127.0.0.1:18082/upload}"
UPLOAD_FILE="${ESPEJISMO_UPLOAD_FILE:-/tmp/espejismo-upload-128m.bin}"
UPLOAD_MIB="${ESPEJISMO_UPLOAD_MIB:-128}"
PARALLEL="${ESPEJISMO_PARALLEL:-4}"
ROUNDS="${ESPEJISMO_ROUNDS:-1}"
ROUND_DELAY_SECS="${ESPEJISMO_ROUND_DELAY_SECS:-5}"
MAX_TIME="${ESPEJISMO_CURL_MAX_TIME:-600}"
ADMIN_URL="${ESPEJISMO_ADMIN_URL:-}"
ADMIN_TOKEN="${ESPEJISMO_ADMIN_TOKEN:-}"
OUTPUT_ROOT="${ESPEJISMO_OUTPUT_DIR:-./bench-results}"
RUN_ID="${ESPEJISMO_RUN_ID:-$(date -u +%Y%m%dT%H%M%SZ)}"
OUTPUT_DIR="${OUTPUT_ROOT%/}/${RUN_ID}"
RAW_DIR="${OUTPUT_DIR}/raw"
RESULTS_JSONL="${OUTPUT_DIR}/results.jsonl"
SUMMARY_MD="${OUTPUT_DIR}/summary.md"

mkdir -p "$RAW_DIR"
: >"$RESULTS_JSONL"

need_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "missing required command: $1" >&2
        exit 127
    fi
}

need_command curl
need_command awk
need_command dd
need_command stat

PYTHON_BIN="${ESPEJISMO_PYTHON:-python3}"
HAS_PYTHON=0
if command -v "$PYTHON_BIN" >/dev/null 2>&1 && "$PYTHON_BIN" -c 'import json, statistics' >/dev/null 2>&1; then
    HAS_PYTHON=1
fi

if [ "$ROUNDS" -lt 1 ]; then
    echo "ESPEJISMO_ROUNDS must be >= 1" >&2
    exit 2
fi

if [ ! -f "$UPLOAD_FILE" ]; then
    echo "creating upload payload: $UPLOAD_FILE (${UPLOAD_MIB} MiB)"
    dd if=/dev/zero of="$UPLOAD_FILE" bs=1M count="$UPLOAD_MIB" status=none
fi

now_ms() {
    date +%s%3N
}

capture_admin() {
    local phase="$1"
    if [ -z "$ADMIN_URL" ]; then
        return 0
    fi
    local out="${OUTPUT_DIR}/admin-${phase}.json"
    if [ -n "$ADMIN_TOKEN" ]; then
        curl -fsS -H "Authorization: Bearer ${ADMIN_TOKEN}" "$ADMIN_URL" -o "$out" || true
    else
        curl -fsS "$ADMIN_URL" -o "$out" || true
    fi
}

metric_value() {
    local file="$1"
    local key="$2"
    awk -F= -v k="$key" '$1 == k {print $2; exit}' "$file"
}

json_escape() {
    printf '%s' "$1" | awk '{
        gsub(/\\/,"\\\\");
        gsub(/"/,"\\\"");
        gsub(/\r/,"\\r");
        gsub(/\t/,"\\t");
        printf "%s", $0;
    }'
}

append_result() {
    local round="$1"
    local case_name="$2"
    local label="$3"
    local mode="$4"
    local direction="$5"
    local parallel="$6"
    local elapsed_secs="$7"
    local bytes="$8"
    local mbps="$9"
    local ok="${10}"
    printf '{"round":%s,"case":"%s","label":"%s","mode":"%s","direction":"%s","parallel":%s,"elapsed_secs":%s,"bytes":%s,"mbps":%s,"ok":%s}\n' \
        "$round" \
        "$(json_escape "$case_name")" \
        "$(json_escape "$label")" \
        "$(json_escape "$mode")" \
        "$(json_escape "$direction")" \
        "$parallel" \
        "$elapsed_secs" \
        "$bytes" \
        "$mbps" \
        "$ok" >>"$RESULTS_JSONL"
}

curl_worker() {
    local label="$1"
    local mode="$2"
    local direction="$3"
    local url="$4"
    local out="${RAW_DIR}/${label}.txt"
    local err="${RAW_DIR}/${label}.stderr"
    local proxy_args=()
    if [ "$mode" = "proxy" ]; then
        proxy_args=(-x "$PROXY_URL")
    fi

    (
        set +e
        if [ "$direction" = "upload" ]; then
            curl -sS -L --max-time "$MAX_TIME" \
                "${proxy_args[@]}" \
                --data-binary @"$UPLOAD_FILE" \
                -o /dev/null \
                -w 'time_total=%{time_total}\nspeed_download=%{speed_download}\nspeed_upload=%{speed_upload}\nhttp_code=%{http_code}\nsize_download=%{size_download}\nsize_upload=%{size_upload}\n' \
                "$url" 2>"$err"
        else
            curl -sS -L --max-time "$MAX_TIME" \
                "${proxy_args[@]}" \
                -o /dev/null \
                -w 'time_total=%{time_total}\nspeed_download=%{speed_download}\nspeed_upload=%{speed_upload}\nhttp_code=%{http_code}\nsize_download=%{size_download}\nsize_upload=%{size_upload}\n' \
                "$url" 2>"$err"
        fi
        local status=$?
        echo "curl_exit=${status}"
        exit 0
    ) >"$out"
}

summarize_one() {
    local round="$1"
    local case_name="$2"
    local label="$3"
    local mode="$4"
    local direction="$5"
    local file="${RAW_DIR}/${label}.txt"
    local elapsed speed bytes exit_code http_code mbps ok
    elapsed="$(metric_value "$file" time_total)"
    exit_code="$(metric_value "$file" curl_exit)"
    http_code="$(metric_value "$file" http_code)"
    if [ "$direction" = "upload" ]; then
        speed="$(metric_value "$file" speed_upload)"
        bytes="$(metric_value "$file" size_upload)"
    else
        speed="$(metric_value "$file" speed_download)"
        bytes="$(metric_value "$file" size_download)"
    fi
    mbps="$(awk -v s="${speed:-0}" 'BEGIN { printf "%.3f", s * 8 / 1000000 }')"
    ok=false
    if [ "${exit_code:-1}" = "0" ] && [ "${http_code:-000}" -ge 200 ] && [ "${http_code:-000}" -lt 400 ]; then
        ok=true
    fi
    append_result "$round" "$case_name" "$label" "$mode" "$direction" 1 "${elapsed:-0}" "${bytes:-0}" "$mbps" "$ok"
    printf '| %s | %s | %s | %s | 1 | %s | %s | %s |\n' "$round" "$case_name" "$mode" "$direction" "${mbps}" "${elapsed:-0}" "$ok" >>"$SUMMARY_MD"
    echo "${label}: ${mbps} Mbit/s (${elapsed:-0}s, ok=${ok})"
}

run_single() {
    local round="$1"
    local case_name="$2"
    local mode="$3"
    local direction="$4"
    local url="$5"
    local label="r${round}-${case_name}"
    curl_worker "$label" "$mode" "$direction" "$url"
    summarize_one "$round" "$case_name" "$label" "$mode" "$direction"
}

run_parallel() {
    local round="$1"
    local case_name="$2"
    local mode="$3"
    local direction="$4"
    local url="$5"
    local label="r${round}-${case_name}"
    local start_ms end_ms elapsed_ms elapsed_secs bytes mbps ok
    start_ms="$(now_ms)"
    for i in $(seq 1 "$PARALLEL"); do
        curl_worker "${label}-${i}" "$mode" "$direction" "$url" &
    done
    wait
    end_ms="$(now_ms)"
    elapsed_ms=$((end_ms - start_ms))
    elapsed_secs="$(awk -v ms="$elapsed_ms" 'BEGIN { printf "%.3f", ms / 1000 }')"
    bytes=0
    ok=true
    for i in $(seq 1 "$PARALLEL"); do
        local file="${RAW_DIR}/${label}-${i}.txt"
        local exit_code http_code size
        exit_code="$(metric_value "$file" curl_exit)"
        http_code="$(metric_value "$file" http_code)"
        if [ "$direction" = "upload" ]; then
            size="$(metric_value "$file" size_upload)"
        else
            size="$(metric_value "$file" size_download)"
        fi
        bytes=$((bytes + ${size:-0}))
        if [ "${exit_code:-1}" != "0" ] || [ "${http_code:-000}" -lt 200 ] || [ "${http_code:-000}" -ge 400 ]; then
            ok=false
        fi
    done
    mbps="$(awk -v bytes="$bytes" -v ms="$elapsed_ms" 'BEGIN { if (ms > 0) printf "%.3f", bytes * 8 / (ms / 1000) / 1000000; else printf "0.000" }')"
    append_result "$round" "$case_name" "$label" "$mode" "$direction" "$PARALLEL" "$elapsed_secs" "$bytes" "$mbps" "$ok"
    printf '| %s | %s | %s | %s | %s | %s | %s | %s |\n' "$round" "$case_name" "$mode" "$direction" "$PARALLEL" "$mbps" "$elapsed_secs" "$ok" >>"$SUMMARY_MD"
    echo "${label}: ${mbps} Mbit/s (${elapsed_secs}s, parallel=${PARALLEL}, ok=${ok})"
}

run_round() {
    local round="$1"
    echo "round ${round}/${ROUNDS}"
    capture_admin "round-${round}-before"
    run_single "$round" direct-download-p1 direct download "$DIRECT_DOWNLOAD_URL"
    run_single "$round" proxy-download-p1 proxy download "$PROXY_DOWNLOAD_URL"
    run_parallel "$round" direct-download-pN direct download "$DIRECT_DOWNLOAD_URL"
    run_parallel "$round" proxy-download-pN proxy download "$PROXY_DOWNLOAD_URL"
    run_single "$round" direct-upload-p1 direct upload "$DIRECT_UPLOAD_URL"
    run_single "$round" proxy-upload-p1 proxy upload "$PROXY_UPLOAD_URL"
    run_parallel "$round" direct-upload-pN direct upload "$DIRECT_UPLOAD_URL"
    run_parallel "$round" proxy-upload-pN proxy upload "$PROXY_UPLOAD_URL"
    capture_admin "round-${round}-after"
}

write_aggregate() {
    if [ "$HAS_PYTHON" -ne 1 ]; then
        cat >>"$SUMMARY_MD" <<EOF

## Aggregate Statistics

Aggregate statistics require a working Python 3 runtime. Set \`ESPEJISMO_PYTHON\`
to a usable interpreter path if \`python3\` is not available.
EOF
        return 0
    fi

    "$PYTHON_BIN" - "$RESULTS_JSONL" "$SUMMARY_MD" <<'PY'
import json
import math
import statistics
import sys
from collections import defaultdict

results_path, summary_path = sys.argv[1], sys.argv[2]
groups = defaultdict(list)
with open(results_path, "r", encoding="utf-8") as fh:
    for line in fh:
        item = json.loads(line)
        groups[item["case"]].append(item)

def fmt(value):
    return f"{value:.3f}"

with open(summary_path, "a", encoding="utf-8") as out:
    out.write("\n## Aggregate Statistics\n\n")
    out.write("| Test | Runs | OK | Median Mbit/s | Mean Mbit/s | Min | Max | Stddev |\n")
    out.write("| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n")
    for case in sorted(groups):
        values = [float(item["mbps"]) for item in groups[case]]
        ok_count = sum(1 for item in groups[case] if item["ok"])
        stddev = statistics.pstdev(values) if len(values) > 1 else 0.0
        out.write(
            f"| {case} | {len(values)} | {ok_count} | {fmt(statistics.median(values))} | "
            f"{fmt(statistics.mean(values))} | {fmt(min(values))} | {fmt(max(values))} | {fmt(stddev)} |\n"
        )

    out.write("\n## Proxy Efficiency\n\n")
    out.write("| Proxy Test | Direct Baseline | Median Efficiency | Mean Efficiency |\n")
    out.write("| --- | --- | ---: | ---: |\n")
    for proxy_case in sorted(case for case in groups if case.startswith("proxy-")):
        direct_case = "direct-" + proxy_case[len("proxy-"):]
        if direct_case not in groups:
            continue
        direct_by_round = {item["round"]: float(item["mbps"]) for item in groups[direct_case]}
        ratios = []
        for item in groups[proxy_case]:
            direct = direct_by_round.get(item["round"], 0.0)
            if direct > 0:
                ratios.append(float(item["mbps"]) / direct * 100.0)
        if ratios:
            out.write(
                f"| {proxy_case} | {direct_case} | {fmt(statistics.median(ratios))}% | "
                f"{fmt(statistics.mean(ratios))}% |\n"
            )
PY
}

cat >"$SUMMARY_MD" <<EOF
# Espejismo Throughput Benchmark

Run: ${RUN_ID}

## Inputs

| Field | Value |
| --- | --- |
| Proxy URL | \`${PROXY_URL}\` |
| Direct download URL | \`${DIRECT_DOWNLOAD_URL}\` |
| Proxy download URL | \`${PROXY_DOWNLOAD_URL}\` |
| Direct upload URL | \`${DIRECT_UPLOAD_URL}\` |
| Proxy upload URL | \`${PROXY_UPLOAD_URL}\` |
| Upload file | \`${UPLOAD_FILE}\` |
| Parallelism | \`${PARALLEL}\` |
| Rounds | \`${ROUNDS}\` |
| Round delay seconds | \`${ROUND_DELAY_SECS}\` |
| Admin URL | \`${ADMIN_URL:-disabled}\` |

## Results

| Round | Test | Mode | Direction | Parallel | Mbit/s | Seconds | OK |
| ---: | --- | --- | --- | ---: | ---: | ---: | --- |
EOF

for round in $(seq 1 "$ROUNDS"); do
    run_round "$round"
    if [ "$round" -lt "$ROUNDS" ] && [ "$ROUND_DELAY_SECS" -gt 0 ]; then
        sleep "$ROUND_DELAY_SECS"
    fi
done

write_aggregate

cat >>"$SUMMARY_MD" <<EOF

## Artifacts

- Raw curl output: \`${RAW_DIR}\`
- JSONL results: \`${RESULTS_JSONL}\`
- Admin snapshots: \`${OUTPUT_DIR}/admin-round-*-before.json\`, \`${OUTPUT_DIR}/admin-round-*-after.json\`
EOF

echo "wrote ${SUMMARY_MD}"
