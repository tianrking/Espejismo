param(
    [string]$Target = "",
    [string]$WintunVersion = "0.14.1"
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

if ($Target -ne "") {
    cargo build --release --locked --target $Target --bin espejismo-local --bin espejismo-remote
    $TargetDir = "target/$Target/release"
} else {
    cargo build --release --locked --bin espejismo-local --bin espejismo-remote
    $Target = (rustc -vV | Select-String "^host:" | ForEach-Object { $_.ToString().Split(" ")[1] })
    $TargetDir = "target/release"
}

$WintunArch = $null
if ($Target -like "*windows*") {
    if ($Target -like "x86_64-*") {
        $WintunArch = "amd64"
    } elseif ($Target -like "i686-*") {
        $WintunArch = "x86"
    } elseif ($Target -like "aarch64-*") {
        $WintunArch = "arm64"
    } else {
        throw "unsupported Windows target for wintun mapping: $Target"
    }
}

$Pkg = "espejismo-$Target"
$Out = "dist/$Pkg"
Remove-Item -Recurse -Force $Out, "dist/$Pkg.zip" -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$Out/bin", "$Out/configs", "$Out/docs", "$Out/scripts" | Out-Null

Copy-Item "$TargetDir/espejismo-local.exe" "$Out/bin/"
Copy-Item "$TargetDir/espejismo-remote.exe" "$Out/bin/"
if ($WintunArch) {
    $tmpZip = Join-Path $env:TEMP "wintun-$WintunVersion.zip"
    $tmpDir = Join-Path $env:TEMP "wintun-$WintunVersion"
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
    Invoke-WebRequest -UseBasicParsing -Uri "https://www.wintun.net/builds/wintun-$WintunVersion.zip" -OutFile $tmpZip
    Expand-Archive -Path $tmpZip -DestinationPath $tmpDir -Force
    $dllPath = Join-Path $tmpDir "wintun\bin\$WintunArch\wintun.dll"
    if (!(Test-Path $dllPath)) {
        throw "wintun.dll not found in archive for arch $WintunArch at $dllPath"
    }
    Copy-Item $dllPath "$Out/bin/wintun.dll"
}
Copy-Item "configs/examples/espejismo.toml" "$Out/configs/"
Copy-Item "README.md" "$Out/"
Copy-Item "README_ES.md" "$Out/"
Copy-Item "CHANGELOG.md" "$Out/"
Copy-Item "docs/ARCHITECTURE.md", "docs/PROTOCOL.md", "docs/deployment/ADMIN.md", "docs/deployment/CLI.md", "docs/deployment/EGRESS.md", "docs/deployment/LOGGING.md", "docs/deployment/PACKAGING.md", "docs/deployment/PROFILES.md", "docs/deployment/QUICKSTART.md", "docs/deployment/TUN.md", "docs/deployment/UPDATES.md", "docs/deployment/USERS.md", "docs/development/STATUS.md", "docs/testing/TEST_PLAN.md" "$Out/docs/"
Copy-Item "scripts/setup-windows.ps1", "scripts/e2e_smoke.sh", "scripts/e2e_smoke.ps1", "scripts/stress_smoke.sh", "scripts/stress_smoke.ps1", "scripts/benchmark_mux.sh", "scripts/install.sh", "scripts/install.ps1", "scripts/install-ubuntu-remote.sh" "$Out/scripts/"

Compress-Archive -Path $Out -DestinationPath "dist/$Pkg.zip" -Force
Write-Host "created dist/$Pkg.zip"
