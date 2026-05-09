param(
    [string]$Target = ""
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

$Pkg = "espejismo-$Target"
$Out = "dist/$Pkg"
Remove-Item -Recurse -Force $Out, "dist/$Pkg.zip" -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force "$Out/bin", "$Out/configs", "$Out/docs", "$Out/scripts" | Out-Null

Copy-Item "$TargetDir/espejismo-local.exe" "$Out/bin/"
Copy-Item "$TargetDir/espejismo-remote.exe" "$Out/bin/"
Copy-Item "configs/examples/espejismo.toml" "$Out/configs/"
Copy-Item "README.md" "$Out/"
Copy-Item "docs/ARCHITECTURE.md", "docs/deployment/ADMIN.md", "docs/deployment/EGRESS.md", "docs/deployment/LOGGING.md", "docs/deployment/PACKAGING.md", "docs/deployment/PROFILES.md", "docs/deployment/QUICKSTART.md", "docs/development/STATUS.md", "docs/testing/TEST_PLAN.md" "$Out/docs/"
Copy-Item "scripts/setup-windows.ps1", "scripts/e2e_smoke.ps1", "scripts/stress_smoke.ps1", "scripts/install-ubuntu-remote.sh" "$Out/scripts/"

Compress-Archive -Path $Out -DestinationPath "dist/$Pkg.zip" -Force
Write-Host "created dist/$Pkg.zip"
