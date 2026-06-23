$ErrorActionPreference = "Stop"

$Repo = if ($env:ESPEJISMO_REPO) { $env:ESPEJISMO_REPO } else { "tianrking/Espejismo" }
$Version = if ($env:ESPEJISMO_VERSION) { $env:ESPEJISMO_VERSION } else { "latest" }
$Package = if ($env:ESPEJISMO_PACKAGE) { $env:ESPEJISMO_PACKAGE } else { "full" }
if ($env:ESPEJISMO_INSTALL_DIR) {
    $InstallDir = $env:ESPEJISMO_INSTALL_DIR
} elseif ($env:LOCALAPPDATA) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Espejismo"
} else {
    $InstallDir = Join-Path $HOME ".espejismo"
}

function Get-EspejismoOs {
    if ($env:ESPEJISMO_OS) { return $env:ESPEJISMO_OS }
    if ($IsLinux) { return "linux" }
    if ($IsMacOS) { return "darwin" }
    return "windows"
}

function Get-EspejismoArch {
    if ($env:ESPEJISMO_ARCH) { return $env:ESPEJISMO_ARCH }
    switch ([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture) {
        "X64" { return "amd64" }
        "X86" { return "386" }
        "Arm64" { return "arm64" }
        "Arm" { return "armv7" }
        default { throw "unsupported architecture: $([System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture)" }
    }
}

$Os = Get-EspejismoOs
$Arch = Get-EspejismoArch

switch ($Package) {
    "full" { $Prefix = "espejismo" }
    "server" { $Prefix = "espejismo-server" }
    default { throw "ESPEJISMO_PACKAGE must be full or server" }
}

$Ext = if ($Os -eq "windows") { "zip" } else { "tar.gz" }
$Artifact = "$Prefix-$Os-$Arch.$Ext"
if ($env:ESPEJISMO_ARCHIVE_URL) {
    $Url = $env:ESPEJISMO_ARCHIVE_URL
} elseif ($Version -eq "latest") {
    $Url = "https://github.com/$Repo/releases/latest/download/$Artifact"
} else {
    $Url = "https://github.com/$Repo/releases/download/$Version/$Artifact"
}

$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("espejismo-install-" + [System.Guid]::NewGuid().ToString("N"))
$Archive = Join-Path $TempDir $Artifact
$OutDir = Join-Path $TempDir "out"
New-Item -ItemType Directory -Force $TempDir, $OutDir, $InstallDir | Out-Null

Write-Host "Downloading $Url"
Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile $Archive

Write-Host "Extracting to $InstallDir"
if ($Ext -eq "zip") {
    Expand-Archive -Force -LiteralPath $Archive -DestinationPath $OutDir
} else {
    $tar = Get-Command tar -ErrorAction SilentlyContinue
    if (-not $tar) {
        throw "tar is required to extract $Artifact"
    }
    & tar -xzf $Archive -C $OutDir
}

$Top = Get-ChildItem -LiteralPath $OutDir -Directory | Select-Object -First 1
if ($Top) {
    Copy-Item -Recurse -Force -Path (Join-Path $Top.FullName "*") -Destination $InstallDir
} else {
    Copy-Item -Recurse -Force -Path (Join-Path $OutDir "*") -Destination $InstallDir
}
Remove-Item -Recurse -Force $TempDir

Write-Host "Installed Espejismo package: $Artifact"
Write-Host "Install directory: $InstallDir"
Write-Host "Binaries:"
$BinDir = Join-Path $InstallDir "bin"
if (Test-Path $BinDir) {
    Get-ChildItem -LiteralPath $BinDir | ForEach-Object { Write-Host "  $($_.FullName)" }
}
Write-Host ""
Write-Host "Next:"
Write-Host "  Server: $BinDir\espejismo-remote.exe --config $InstallDir\configs\espejismo.toml"
Write-Host "  Client: $BinDir\espejismo-local.exe --config $InstallDir\configs\espejismo.toml"
