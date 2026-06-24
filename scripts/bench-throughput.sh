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
ENV_MD="${OUTPUT_DIR}/environment.md"
LOG_RISK_MD="${OUTPUT_DIR}/log-risk.md"
LOCAL_LOG_FILE="${ESPEJISMO_LOCAL_LOG_FILE:-/tmp/espejismo-local-bench.log}"
LOG_SCAN_LINES="${ESPEJISMO_LOG_SCAN_LINES:-300}"
MAX_LOG_LINE_BYTES="${ESPEJISMO_MAX_LOG_LINE_BYTES:-8192}"
ALLOW_VERBOSE_LOGS="${ESPEJISMO_ALLOW_VERBOSE_LOGS:-0}"

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

command_output() {
    local title="$1"
    shift
    {
        echo "### ${title}"
        echo
        echo '```text'
        "$@" 2>&1 || true
        echo '```'
        echo
    } >>"$ENV_MD"
}

capture_environment() {
    cat >"$ENV_MD" <<EOF
# Espejismo Benchmark Environment

Run: ${RUN_ID}

| Field | Value |
| --- | --- |
| Started UTC | \`$(date -u +%Y-%m-%dT%H:%M:%SZ)\` |
| Host | \`$(hostname 2>/dev/null || echo unknown)\` |
| Proxy URL | \`${PROXY_URL}\` |
| Direct download URL | \`${DIRECT_DOWNLOAD_URL}\` |
| Proxy download URL | \`${PROXY_DOWNLOAD_URL}\` |
| Direct upload URL | \`${DIRECT_UPLOAD_URL}\` |
| Proxy upload URL | \`${PROXY_UPLOAD_URL}\` |
| Upload file | \`${UPLOAD_FILE}\` |
| Upload MiB | \`${UPLOAD_MIB}\` |
| Parallelism | \`${PARALLEL}\` |
| Rounds | \`${ROUNDS}\` |
| Curl max time | \`${MAX_TIME}\` |
| Admin URL | \`${ADMIN_URL:-disabled}\` |
| Local log file scanned | \`${LOCAL_LOG_FILE}\` |
| Log scan lines | \`${LOG_SCAN_LINES}\` |
| Max allowed log line bytes | \`${MAX_LOG_LINE_BYTES}\` |

EOF
    command_output "Kernel" uname -a
    command_output "Curl" curl --version
    if [ "$HAS_PYTHON" -eq 1 ]; then
        command_output "Python" "$PYTHON_BIN" --version
    fi
    if command -v git >/dev/null 2>&1 && git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        command_output "Git" git log --oneline -1
    fi
    if [ -n "$ADMIN_URL" ]; then
        local admin_out="${OUTPUT_DIR}/admin-environment.json"
        if [ -n "$ADMIN_TOKEN" ]; then
            curl -fsS -H "Authorization: Bearer ${ADMIN_TOKEN}" "$ADMIN_URL" -o "$admin_out" || true
        else
            curl -fsS "$ADMIN_URL" -o "$admin_out" || true
        fi
        if [ -s "$admin_out" ]; then
            {
                echo "### Admin Status"
                echo
                echo "Captured in \`${admin_out}\`."
                echo
            } >>"$ENV_MD"
        fi
    fi
}

check_log_safety() {
    cat >"$LOG_RISK_MD" <<EOF
# Espejismo Benchmark Log Risk Check

| Field | Value |
| --- | --- |
| Log file | \`${LOCAL_LOG_FILE}\` |
| Lines scanned from tail | \`${LOG_SCAN_LINES}\` |
| Max allowed line bytes | \`${MAX_LOG_LINE_BYTES}\` |
| Allow verbose logs | \`${ALLOW_VERBOSE_LOGS}\` |

EOF

    if [ ! -f "$LOCAL_LOG_FILE" ]; then
        cat >>"$LOG_RISK_MD" <<EOF
No log file was found at the configured path. The benchmark will continue.
EOF
        return 0
    fi

    local scan_file="${RAW_DIR}/log-scan-tail.txt"
    tail -n "$LOG_SCAN_LINES" "$LOCAL_LOG_FILE" >"$scan_file" || true
    local max_line giant_lines frame_dumps
    max_line="$(awk '{ if (length($0) > max) max = length($0) } END { print max + 0 }' "$scan_file")"
    giant_lines="$(awk -v limit="$MAX_LOG_LINE_BYTES" 'length($0) > limit { count++ } END { print count + 0 }' "$scan_file")"
    frame_dumps="$( (grep -E 'tokio_yamux::session|Frame \{.*body: Some' "$scan_file" || true) | wc -l | awk '{print $1}' )"

    cat >>"$LOG_RISK_MD" <<EOF
| Maximum observed line bytes | \`${max_line}\` |
| Lines over limit | \`${giant_lines}\` |
| Suspected frame-dump lines | \`${frame_dumps}\` |
| Tail sample | \`${scan_file}\` |

EOF

    if [ "$giant_lines" -gt 0 ] || [ "$frame_dumps" -gt 0 ]; then
        cat >>"$LOG_RISK_MD" <<EOF
Result: unsafe for throughput measurement. Verbose frame logs can dominate I/O
and make proxy transfer results look much slower than the protocol really is.
Restart Espejismo with \`[logging] level = "info"\` or an application-only debug
filter before running the benchmark.
EOF
        if [ "$ALLOW_VERBOSE_LOGS" != "1" ]; then
            echo "unsafe verbose logs detected; see ${LOG_RISK_MD}" >&2
            exit 3
        fi
    else
        cat >>"$LOG_RISK_MD" <<EOF
Result: safe. No recent giant log lines or mux frame body dumps were detected.
EOF
    fi
}

now_ms() {
    local value
    value="$(date +%s%3N 2>/dev/null || true)"
    if [[ "$value" =~ ^[0-9]+$ ]]; then
        echo "$value"
    elif [ "$HAS_PYTHON" -eq 1 ]; then
        "$PYTHON_BIN" - <<'PY'
import time
print(int(time.time() * 1000))
PY
    else
        echo "$(($(date +%s) * 1000))"
    fi
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
    capture_admin "${label}-before"
    curl_worker "$label" "$mode" "$direction" "$url"
    summarize_one "$round" "$case_name" "$label" "$mode" "$direction"
    capture_admin "${label}-after"
}

run_parallel() {
    local round="$1"
    local case_name="$2"
    local mode="$3"
    local direction="$4"
    local url="$5"
    local label="r${round}-${case_name}"
    local start_ms end_ms elapsed_ms elapsed_secs bytes mbps ok
    capture_admin "${label}-before"
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
    capture_admin "${label}-after"
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

    "$PYTHON_BIN" - "$RESULTS_JSONL" "$SUMMARY_MD" "$OUTPUT_DIR" <<'PY'
import json
import math
from pathlib import Path
import statistics
import sys
from collections import defaultdict

results_path, summary_path, output_dir = sys.argv[1], sys.argv[2], Path(sys.argv[3])
groups = defaultdict(list)
results = []
with open(results_path, "r", encoding="utf-8") as fh:
    for line in fh:
        item = json.loads(line)
        results.append(item)
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

    def read_admin(label, suffix):
        path = output_dir / f"admin-{label}-{suffix}.json"
        if not path.exists() or path.stat().st_size == 0:
            return None
        try:
            return json.loads(path.read_text(encoding="utf-8"))
        except Exception:
            return None

    def tunnel_counters(snapshot):
        runtime = snapshot.get("runtime") or {}
        lanes = runtime.get("tunnel_lanes") or []
        if lanes:
            return {
                "client_to_remote": sum(int(lane.get("bytes_client_to_remote") or 0) for lane in lanes),
                "remote_to_client": sum(int(lane.get("bytes_remote_to_client") or 0) for lane in lanes),
                "source": "runtime.tunnel_lanes",
            }
        metrics = snapshot.get("metrics") or {}
        return {
            "client_to_remote": int(metrics.get("bytes_client_to_remote") or 0),
            "remote_to_client": int(metrics.get("bytes_remote_to_client") or 0),
            "source": "metrics",
        }

    out.write("\n## Tunnel Cost\n\n")
    out.write(
        "This section compares application bytes reported by curl with local "
        "Espejismo tunnel byte deltas from per-test admin snapshots. DATA bytes, "
        "control frames, encryption overhead, stealth padding, and idle padding "
        "can all contribute to the tunnel delta.\n\n"
    )
    out.write("| Proxy Test | Direction | App Bytes | Tunnel Primary Bytes | Ratio | Reverse Bytes | Source |\n")
    out.write("| --- | --- | ---: | ---: | ---: | ---: | --- |\n")
    rows = 0
    for item in results:
        if item["mode"] != "proxy":
            continue
        before = read_admin(item["label"], "before")
        after = read_admin(item["label"], "after")
        if before is None or after is None:
            continue
        b = tunnel_counters(before)
        a = tunnel_counters(after)
        up = max(0, a["client_to_remote"] - b["client_to_remote"])
        down = max(0, a["remote_to_client"] - b["remote_to_client"])
        if item["direction"] == "upload":
            primary, reverse = up, down
        else:
            primary, reverse = down, up
        app_bytes = int(item["bytes"])
        ratio = (primary / app_bytes * 100.0) if app_bytes > 0 else math.nan
        ratio_text = "n/a" if math.isnan(ratio) else f"{fmt(ratio)}%"
        out.write(
            f"| {item['label']} | {item['direction']} | {app_bytes} | {primary} | "
            f"{ratio_text} | {reverse} | {a['source']} |\n"
        )
        rows += 1
    if rows == 0:
        out.write("| n/a | n/a | 0 | 0 | n/a | 0 | admin snapshots missing |\n")
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

Environment: \`${ENV_MD}\`

Log risk check: \`${LOG_RISK_MD}\`

## Results

| Round | Test | Mode | Direction | Parallel | Mbit/s | Seconds | OK |
| ---: | --- | --- | --- | ---: | ---: | ---: | --- |
EOF

capture_environment
check_log_safety

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
- Admin snapshots: \`${OUTPUT_DIR}/admin-*.json\`
EOF

echo "wrote ${SUMMARY_MD}"
