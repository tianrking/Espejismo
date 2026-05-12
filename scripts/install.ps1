param(
    [string]$Role = $env:ESPEJISMO_ROLE,
    [string]$Repo = $(if ($env:ESPEJISMO_REPO) { $env:ESPEJISMO_REPO } else { "tianrking/Espejismo" }),
    [string]$Version = $(if ($env:ESPEJISMO_VERSION) { $env:ESPEJISMO_VERSION } else { "latest" }),
    [string]$ArchiveUrl = $env:ESPEJISMO_ARCHIVE_URL,
    [string]$InstallDir = $env:ESPEJISMO_INSTALL_DIR,
    [string]$Server = $env:ESPEJISMO_SERVER,
    [string]$Listen = $(if ($env:ESPEJISMO_LISTEN) { $env:ESPEJISMO_LISTEN } else { "0.0.0.0:6690" }),
    [string]$PublicEndpoint = $env:ESPEJISMO_PUBLIC_ENDPOINT,
    [string]$Socks5Listen = $(if ($env:ESPEJISMO_SOCKS5_LISTEN) { $env:ESPEJISMO_SOCKS5_LISTEN } else { "127.0.0.1:6680" }),
    [string]$HttpListen = $(if ($env:ESPEJISMO_HTTP_LISTEN) { $env:ESPEJISMO_HTTP_LISTEN } else { "127.0.0.1:6681" }),
    [string]$Psk = $env:ESPEJISMO_PSK,
    [string]$AdminToken = $env:ESPEJISMO_ADMIN_TOKEN,
    [string]$LocalUser = $(if ($env:ESPEJISMO_LOCAL_AUTH_USER) { $env:ESPEJISMO_LOCAL_AUTH_USER } else { "local-user" }),
    [string]$LocalPassword = $env:ESPEJISMO_LOCAL_AUTH_PASSWORD,
    [switch]$NoStart
)

$ErrorActionPreference = "Stop"

function New-Secret {
    $bytes = New-Object byte[] 32
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    [Convert]::ToBase64String($bytes)
}

function Ask-Default([string]$Prompt, [string]$Default) {
    $value = Read-Host "$Prompt [$Default]"
    if ([string]::IsNullOrWhiteSpace($value)) { $Default } else { $value }
}

function Escape-Toml([string]$Value) {
    if ($null -eq $Value) { "" } else { $Value.Replace("\", "\\").Replace('"', '\"') }
}

function Escape-PowerShellSingle([string]$Value) {
    if ($null -eq $Value) { "" } else { $Value.Replace("'", "''") }
}

function Package-Name {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        "X64" { "espejismo-windows-amd64" }
        "X86" { "espejismo-windows-386" }
        "Arm64" { "espejismo-windows-arm64" }
        default { throw "unsupported Windows architecture: $arch" }
    }
}

if ([string]::IsNullOrWhiteSpace($Role)) {
    $choice = Ask-Default "Install mode: local or remote" "local"
    if ($choice -eq "remote") { $Role = "remote" } else { $Role = "local" }
}
if ($Role -ne "local" -and $Role -ne "remote") {
    throw "Role must be local or remote"
}

if ([string]::IsNullOrWhiteSpace($Psk)) { $Psk = New-Secret }
if ([string]::IsNullOrWhiteSpace($AdminToken)) { $AdminToken = New-Secret }
if ($Role -eq "remote") {
    $Listen = Ask-Default "Remote listen address" $Listen
    if ([string]::IsNullOrWhiteSpace($PublicEndpoint)) {
        $port = ($Listen -split ":")[-1]
        $PublicEndpoint = Ask-Default "Public client endpoint" "127.0.0.1:$port"
    }
    $Server = $PublicEndpoint
} else {
    $Server = Ask-Default "Remote server endpoint host:port" $(if ($Server) { $Server } else { "127.0.0.1:6690" })
    $Socks5Listen = Ask-Default "Local SOCKS5 listen" $Socks5Listen
    $HttpListen = Ask-Default "Local HTTP proxy listen" $HttpListen
}

if ([string]::IsNullOrWhiteSpace($InstallDir)) {
    $InstallDir = Join-Path $env:LOCALAPPDATA "Espejismo"
}
$BinDir = Join-Path $InstallDir "bin"
$ConfigDir = Join-Path $InstallDir "config"
$ConfigPath = Join-Path $ConfigDir "espejismo.toml"
$LogPath = Join-Path $ConfigDir "espejismo-$Role.log"
$PidPath = Join-Path $ConfigDir "espejismo-$Role.pid"
$CtlPath = Join-Path $InstallDir "espejismoctl.ps1"
$AdminListen = if ($Role -eq "remote") { "127.0.0.1:9090" } else { "127.0.0.1:9091" }
$LocalAuthToml = ""
if (-not [string]::IsNullOrWhiteSpace($LocalPassword)) {
    $LocalAuthToml = @"

[local.auth]
username = "$(Escape-Toml $LocalUser)"
password = "$(Escape-Toml $LocalPassword)"
"@
}
$LocalConfigToml = @"

[local]
server = "$(Escape-Toml $Server)"
socks5_listen = "$Socks5Listen"
http_listen = "$HttpListen"
handshake_padding = 256
$LocalAuthToml

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3
max_connection_age_secs = 3600
"@
$RemoteConfigToml = @"

[remote]
listen = "$Listen"
handshake_timeout_ms = 3000
reject_delay_ms = 0
max_handshake_padding = 1024
replay_window_secs = 60
cold_start_delay_ms = 35
tarpit_max = 1024
tarpit_hold_secs = 300

[remote.egress]
deny_private_ips = true
allow_ports = [80, 443]
block_ports = [25]
block_hosts = ["169.254.169.254", "metadata.google.internal"]
"@
$RoleConfigToml = if ($Role -eq "remote") { $RemoteConfigToml } else { $LocalConfigToml }

New-Item -ItemType Directory -Force $BinDir, $ConfigDir | Out-Null

$pkg = Package-Name
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("espejismo-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force $tmp | Out-Null
try {
    $archive = Join-Path $tmp "$pkg.zip"
    if ([string]::IsNullOrWhiteSpace($ArchiveUrl)) {
        if ($Version -eq "latest") {
            $ArchiveUrl = "https://github.com/$Repo/releases/latest/download/$pkg.zip"
        } else {
            $ArchiveUrl = "https://github.com/$Repo/releases/download/$Version/$pkg.zip"
        }
    }
    Write-Host "Downloading $pkg from $ArchiveUrl"
    Invoke-WebRequest -Uri $ArchiveUrl -OutFile $archive
    Expand-Archive -Path $archive -DestinationPath $tmp -Force
    $pkgDir = Get-ChildItem $tmp -Directory | Where-Object { $_.Name -like "espejismo-*" } | Select-Object -First 1
    if (-not $pkgDir) { throw "invalid release archive" }
    Copy-Item (Join-Path $pkgDir.FullName "bin/espejismo-local.exe") $BinDir -Force
    Copy-Item (Join-Path $pkgDir.FullName "bin/espejismo-remote.exe") $BinDir -Force
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

$config = @"
[shared]
psk = "$(Escape-Toml $Psk)"
clock_skew_secs = 30
puzzle_bits = 12
max_padding = 64
jitter_ms = 0
padding_chance_percent = 35
tunnel_buffer = 1048576
idle_timeout_secs = 300
max_streams = 256
max_physical_connections = 1024
key_update_frames = 16384

[shared.tcp]
nodelay = true
keepalive_secs = 30
heartbeat_secs = 30
user_timeout_ms = 30000
send_buffer_bytes = 1048576
recv_buffer_bytes = 1048576

[shared.mux]
mode = "yamux"
native_initial_window_bytes = 1048576
native_stream_buffer_frames = 128
native_send_queue_frames = 64
native_idle_timeout_secs = 300
native_drain_timeout_secs = 30

[shared.pacing]
enabled = true
max_bytes_per_sec = 0
burst_bytes = 65536
min_write_bytes = 1024

[shared.obfuscation]
profile = "balanced"
chunk_policy = "balanced"
randomize_chunks = true
min_chunk = 4096
max_chunk = 16384

[logging]
level = "info"
format = "compact"
ansi = false
file = "$(Escape-Toml $LogPath)"

[admin]
listen = "$AdminListen"
token = "$(Escape-Toml $AdminToken)"
$RoleConfigToml
"@
Set-Content -Path $ConfigPath -Value $config -Encoding UTF8

$binary = if ($Role -eq "remote") { "espejismo-remote.exe" } else { "espejismo-local.exe" }
$ctl = @"
param([string]`$Command = "status")
`$ErrorActionPreference = "Stop"
`$Role = "$Role"
`$Bin = "$((Join-Path $BinDir $binary).Replace('\', '\\'))"
`$Config = "$($ConfigPath.Replace('\', '\\'))"
`$PidFile = "$($PidPath.Replace('\', '\\'))"
`$LogFile = "$($LogPath.Replace('\', '\\'))"
`$Admin = "http://$AdminListen"
`$Token = "$AdminToken"
`$ServerEndpoint = '$(Escape-PowerShellSingle $Server)'
`$Socks5Addr = '$(Escape-PowerShellSingle $Socks5Listen)'
`$HttpAddr = '$(Escape-PowerShellSingle $HttpListen)'
`$LocalAuthUser = '$(Escape-PowerShellSingle $LocalUser)'
`$LocalAuthPassword = '$(Escape-PowerShellSingle $LocalPassword)'

function Start-Espejismo {
    if (Test-Path `$PidFile) {
        `$pidValue = Get-Content `$PidFile -ErrorAction SilentlyContinue
        if (`$pidValue -and (Get-Process -Id `$pidValue -ErrorAction SilentlyContinue)) {
            Write-Host "`$Role already running: `$pidValue"
            return
        }
    }
    `$p = Start-Process -FilePath `$Bin -ArgumentList @("--config", `$Config) -RedirectStandardOutput `$LogFile -RedirectStandardError `$LogFile -WindowStyle Hidden -PassThru
    Set-Content -Path `$PidFile -Value `$p.Id
    Write-Host "started `$Role: `$(`$p.Id)"
}

function Stop-Espejismo {
    if (Test-Path `$PidFile) {
        `$pidValue = Get-Content `$PidFile
        Stop-Process -Id `$pidValue -Force -ErrorAction SilentlyContinue
        Remove-Item `$PidFile -Force -ErrorAction SilentlyContinue
    }
    Write-Host "stopped `$Role"
}

function Status-Espejismo {
    if (Test-Path `$PidFile) {
        `$pidValue = Get-Content `$PidFile
        if (`$pidValue -and (Get-Process -Id `$pidValue -ErrorAction SilentlyContinue)) {
            Write-Host "`$Role running: `$pidValue"
        } else {
            Write-Host "`$Role stopped"
        }
    } else {
        Write-Host "`$Role stopped"
    }
    try {
        Invoke-RestMethod -Headers @{ Authorization = "Bearer `$Token" } -Uri "`$Admin/status" | ConvertTo-Json -Depth 8
    } catch {}
}

function Connect-Info {
    if (`$Role -eq "local") {
        Write-Host "Local proxy is ready."
        Write-Host "  SOCKS5: `$Socks5Addr"
        Write-Host "  HTTP:   `$HttpAddr"
        Write-Host ""
        Write-Host "Test commands:"
        if (`$LocalAuthPassword) {
            Write-Host "  curl.exe --proxy-user ""`$LocalAuthUser`:`$LocalAuthPassword"" --socks5-hostname ""`$Socks5Addr"" https://ifconfig.me"
            Write-Host "  curl.exe --proxy-user ""`$LocalAuthUser`:`$LocalAuthPassword"" -x ""http://`$HttpAddr"" https://ifconfig.me"
        } else {
            Write-Host "  curl.exe --socks5-hostname ""`$Socks5Addr"" https://ifconfig.me"
            Write-Host "  curl.exe -x ""http://`$HttpAddr"" https://ifconfig.me"
        }
        Write-Host ""
        Write-Host "Browser/app settings:"
        Write-Host "  SOCKS5 host/port: `$Socks5Addr"
        Write-Host "  HTTP proxy:       `$HttpAddr"
        if (`$LocalAuthPassword) {
            Write-Host "  Proxy auth:       `$LocalAuthUser / `$LocalAuthPassword"
        } else {
            Write-Host "  Proxy auth:       disabled"
        }
    } else {
        `$tmpConfig = [System.IO.Path]::GetTempFileName()
        Copy-Item `$Config `$tmpConfig -Force
        Add-Content -Path `$tmpConfig -Value @"

[local]
server = "`$ServerEndpoint"
socks5_listen = "`$Socks5Addr"
http_listen = "`$HttpAddr"
handshake_padding = 256

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3
max_connection_age_secs = 3600
"@
        if (`$LocalAuthPassword) {
            Add-Content -Path `$tmpConfig -Value @"

[local.auth]
username = "`$LocalAuthUser"
password = "`$LocalAuthPassword"
"@
        }
        try {
            `$profileUrl = & (Join-Path (Split-Path `$Bin) "espejismo-local.exe") --config `$tmpConfig --print-client-profile --profile-name default
        } finally {
            Remove-Item `$tmpConfig -Force -ErrorAction SilentlyContinue
        }
        Write-Host "Remote endpoint is ready."
        Write-Host "  Public endpoint: `$ServerEndpoint"
        Write-Host ""
        Write-Host "Client import profile:"
        Write-Host "  `$profileUrl"
        Write-Host ""
        Write-Host "Client one-line start:"
        Write-Host "  espejismo-local --import-profile '`$profileUrl'"
    }
}

switch (`$Command) {
    "start" { Start-Espejismo }
    "stop" { Stop-Espejismo }
    "restart" { Stop-Espejismo; Start-Sleep -Seconds 1; Start-Espejismo }
    "status" { Status-Espejismo }
    "reload" { Invoke-RestMethod -Method Post -Headers @{ Authorization = "Bearer `$Token" } -Uri "`$Admin/reload" | ConvertTo-Json -Depth 8 }
    "logs" { Get-Content `$LogFile -Wait }
    "edit" { notepad `$Config }
    "profile" { & (Join-Path (Split-Path `$Bin) "espejismo-local.exe") --config `$Config --print-client-profile --profile-name default }
    "connect" { Connect-Info }
    "config" { Write-Host `$Config }
    default { Write-Host "usage: .\espejismoctl.ps1 start|stop|restart|status|reload|logs|edit|profile|connect|config"; exit 2 }
}
"@
Set-Content -Path $CtlPath -Value $ctl -Encoding UTF8

if (-not $NoStart) {
    & powershell -ExecutionPolicy Bypass -File $CtlPath restart
}

Write-Host ""
Write-Host "Espejismo $Role installed."
Write-Host "  Install dir: $InstallDir"
Write-Host "  Config:      $ConfigPath"
Write-Host "  Manager:     $CtlPath"
Write-Host ""
Write-Host "Management:"
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$CtlPath`" status"
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$CtlPath`" logs"
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$CtlPath`" edit"
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$CtlPath`" restart"
Write-Host "  powershell -ExecutionPolicy Bypass -File `"$CtlPath`" connect"
Write-Host ""
Write-Host "Connection:"
& powershell -ExecutionPolicy Bypass -File $CtlPath connect
