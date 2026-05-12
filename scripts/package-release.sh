#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TARGET="${1:-}"

cd "${ROOT}"

if [[ -n "${TARGET}" ]]; then
  cargo build --release --locked --target "${TARGET}" --bin espejismo-local --bin espejismo-remote
  TARGET_DIR="target/${TARGET}/release"
else
  cargo build --release --locked --bin espejismo-local --bin espejismo-remote
  TARGET="$(rustc -vV | awk '/host:/ {print $2}')"
  TARGET_DIR="target/release"
fi

PKG="espejismo-${TARGET}"
OUT="dist/${PKG}"
SERVER_PKG="espejismo-server-${TARGET}"
SERVER_OUT="dist/${SERVER_PKG}"
rm -rf "${OUT}" "${SERVER_OUT}" "dist/${PKG}.tar.gz" "dist/${SERVER_PKG}.tar.gz"
mkdir -p "${OUT}/bin" "${OUT}/configs" "${OUT}/docs" "${OUT}/scripts"

cp "${TARGET_DIR}/espejismo-local" "${OUT}/bin/"
cp "${TARGET_DIR}/espejismo-remote" "${OUT}/bin/"
cp "configs/examples/espejismo.toml" "${OUT}/configs/"
cp "README.md" "${OUT}/"
cp "README_ES.md" "${OUT}/"
cp "CHANGELOG.md" "${OUT}/"
cp "docs/ARCHITECTURE.md" "docs/PROTOCOL.md" "docs/deployment/ADMIN.md" "docs/deployment/CLI.md" "docs/deployment/EGRESS.md" "docs/deployment/LOGGING.md" "docs/deployment/PACKAGING.md" "docs/deployment/PROFILES.md" "docs/deployment/QUICKSTART.md" "docs/deployment/TUN.md" "docs/deployment/UPDATES.md" "docs/deployment/USERS.md" "docs/development/STATUS.md" "docs/testing/TEST_PLAN.md" "${OUT}/docs/"
cp "scripts/setup-windows.ps1" "scripts/e2e_smoke.sh" "scripts/e2e_smoke.ps1" "scripts/stress_smoke.sh" "scripts/stress_smoke.ps1" "scripts/benchmark_mux.sh" "scripts/install.sh" "scripts/install.ps1" "scripts/install-ubuntu-remote.sh" "${OUT}/scripts/"

tar -C dist -czf "dist/${PKG}.tar.gz" "${PKG}"
echo "created dist/${PKG}.tar.gz"

mkdir -p "${SERVER_OUT}/bin" "${SERVER_OUT}/configs" "${SERVER_OUT}/docs" "${SERVER_OUT}/scripts"
cp "${TARGET_DIR}/espejismo-remote" "${SERVER_OUT}/bin/"
cp "configs/examples/espejismo.toml" "${SERVER_OUT}/configs/"
cp "README.md" "CHANGELOG.md" "${SERVER_OUT}/"
cp "docs/deployment/ADMIN.md" "docs/deployment/EGRESS.md" "docs/deployment/LOGGING.md" "docs/deployment/QUICKSTART.md" "docs/deployment/USERS.md" "${SERVER_OUT}/docs/"
cp "scripts/install-ubuntu-remote.sh" "${SERVER_OUT}/scripts/"

tar -C dist -czf "dist/${SERVER_PKG}.tar.gz" "${SERVER_PKG}"
echo "created dist/${SERVER_PKG}.tar.gz"
