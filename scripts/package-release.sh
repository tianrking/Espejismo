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
rm -rf "${OUT}" "dist/${PKG}.tar.gz"
mkdir -p "${OUT}/bin" "${OUT}/configs" "${OUT}/docs" "${OUT}/scripts"

cp "${TARGET_DIR}/espejismo-local" "${OUT}/bin/"
cp "${TARGET_DIR}/espejismo-remote" "${OUT}/bin/"
cp "configs/examples/espejismo.toml" "${OUT}/configs/"
cp "README.md" "${OUT}/"
cp "docs/ARCHITECTURE.md" "docs/deployment/ADMIN.md" "docs/deployment/EGRESS.md" "docs/deployment/LOGGING.md" "docs/deployment/PACKAGING.md" "docs/deployment/PROFILES.md" "docs/deployment/QUICKSTART.md" "docs/deployment/UPDATES.md" "docs/deployment/USERS.md" "docs/development/STATUS.md" "docs/testing/TEST_PLAN.md" "${OUT}/docs/"
cp "scripts/setup-windows.ps1" "scripts/e2e_smoke.ps1" "scripts/stress_smoke.ps1" "scripts/install-ubuntu-remote.sh" "${OUT}/scripts/"

tar -C dist -czf "dist/${PKG}.tar.gz" "${PKG}"
echo "created dist/${PKG}.tar.gz"
