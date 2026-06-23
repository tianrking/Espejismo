#!/usr/bin/env sh
set -eu

repo="${ESPEJISMO_REPO:-tianrking/Espejismo}"
version="${ESPEJISMO_VERSION:-latest}"
package="${ESPEJISMO_PACKAGE:-full}"
install_dir="${ESPEJISMO_INSTALL_DIR:-$HOME/.espejismo}"

need() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

detect_os() {
  case "$(uname -s 2>/dev/null | tr '[:upper:]' '[:lower:]')" in
    linux*) echo "linux" ;;
    darwin*) echo "darwin" ;;
    mingw*|msys*|cygwin*) echo "windows" ;;
    *) echo "unsupported" ;;
  esac
}

detect_arch() {
  case "$(uname -m 2>/dev/null | tr '[:upper:]' '[:lower:]')" in
    x86_64|amd64) echo "amd64" ;;
    i386|i686) echo "386" ;;
    aarch64|arm64) echo "arm64" ;;
    armv7l|armv7*) echo "armv7" ;;
    *) echo "unsupported" ;;
  esac
}

download() {
  url="$1"
  out="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fL "$url" -o "$out"
  elif command -v wget >/dev/null 2>&1; then
    wget -O "$out" "$url"
  else
    echo "missing required command: curl or wget" >&2
    exit 1
  fi
}

os="${ESPEJISMO_OS:-$(detect_os)}"
arch="${ESPEJISMO_ARCH:-$(detect_arch)}"
if [ "$os" = "unsupported" ] || [ "$arch" = "unsupported" ]; then
  echo "unsupported platform: os=$os arch=$arch" >&2
  exit 1
fi

case "$package" in
  full) prefix="espejismo" ;;
  server) prefix="espejismo-server" ;;
  *) echo "ESPEJISMO_PACKAGE must be full or server" >&2; exit 1 ;;
esac

case "$os" in
  windows) ext="zip" ;;
  *) ext="tar.gz" ;;
esac

artifact="${prefix}-${os}-${arch}.${ext}"
if [ -n "${ESPEJISMO_ARCHIVE_URL:-}" ]; then
  url="$ESPEJISMO_ARCHIVE_URL"
else
  if [ "$version" = "latest" ]; then
    url="https://github.com/${repo}/releases/latest/download/${artifact}"
  else
    url="https://github.com/${repo}/releases/download/${version}/${artifact}"
  fi
fi

tmp_dir="${TMPDIR:-/tmp}/espejismo-install-$$"
mkdir -p "$tmp_dir" "$install_dir"
archive="$tmp_dir/$artifact"

echo "Downloading $url"
download "$url" "$archive"

echo "Extracting to $install_dir"
if [ "$ext" = "zip" ]; then
  if command -v unzip >/dev/null 2>&1; then
    unzip -oq "$archive" -d "$tmp_dir/out"
  elif command -v powershell.exe >/dev/null 2>&1; then
    powershell.exe -NoProfile -Command "Expand-Archive -Force -LiteralPath '$archive' -DestinationPath '$tmp_dir/out'"
  else
    echo "missing unzip or powershell.exe for zip extraction" >&2
    exit 1
  fi
else
  need tar
  tar -xzf "$archive" -C "$tmp_dir"
  mkdir -p "$tmp_dir/out"
  top="$(find "$tmp_dir" -mindepth 1 -maxdepth 1 -type d ! -name out | head -n 1)"
  cp -R "$top"/. "$tmp_dir/out/"
fi

if [ "$ext" = "zip" ]; then
  top="$(find "$tmp_dir/out" -mindepth 1 -maxdepth 1 -type d | head -n 1 || true)"
  if [ -n "$top" ]; then
    cp -R "$top"/. "$install_dir/"
  else
    cp -R "$tmp_dir/out"/. "$install_dir/"
  fi
else
  cp -R "$tmp_dir/out"/. "$install_dir/"
fi

rm -rf "$tmp_dir"

echo "Installed Espejismo package: $artifact"
echo "Install directory: $install_dir"
echo "Binaries:"
find "$install_dir/bin" -maxdepth 1 -type f 2>/dev/null | sed 's/^/  /' || true
echo
echo "Next:"
if [ "$os" = "windows" ]; then
  echo "  Server: $install_dir/bin/espejismo-remote.exe --config $install_dir/configs/espejismo.toml"
  echo "  Client: $install_dir/bin/espejismo-local.exe --config $install_dir/configs/espejismo.toml"
else
  echo "  Server: $install_dir/bin/espejismo-remote --config $install_dir/configs/espejismo.toml"
  echo "  Client: $install_dir/bin/espejismo-local --config $install_dir/configs/espejismo.toml"
fi
