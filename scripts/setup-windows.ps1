param(
    [ValidateSet("local", "remote")]
    [string]$Mode = "local",
    [string]$InstallDir = "",
    [string]$ConfigPath = "",
    [string]$ProfileUrl = "",
    [string]$Server = "",
    [string]$Psk = "",
    [string]$RemoteListen = "0.0.0.0:6690",
    [string]$Socks5Listen = "127.0.0.1:6680",
    [string]$HttpListen = "127.0.0.1:6681",
    [string]$ProxyUsername = "",
    [string]$ProxyPassword = "",
    [string]$AdminListen = "",
    [string]$AdminToken = "",
    [int]$ClockSkewSecs = 30,
    [int]$PuzzleBits = 12,
    [int]$MaxPadding = 64,
    [int]$JitterMs = 0,
    [int]$PaddingChancePercent = 35,
    [int]$BackpressureThresholdMs = 40,
    [int]$BackpressureCooldownMs = 1000,
    [int]$TunnelBuffer = 1048576,
    [int]$IdleTimeoutSecs = 300,
    [int]$MaxStreams = 256,
    [ValidateSet("low_latency", "balanced", "high_entropy")]
    [string]$ObfuscationProfile = "balanced",
    [bool]$RandomizeChunks = $true,
    [int]$MinChunk = 1024,
    [int]$MaxChunk = 16384,
    [int]$HandshakePadding = 256,
    [int]$HandshakeTimeoutMs = 3000,
    [int]$RejectDelayMs = 0,
    [int]$MaxHandshakePadding = 1024,
    [int]$ReplayWindowSecs = 60,
    [int]$ColdStartDelayMs = 35,
    [int]$TarpitMax = 1024,
    [int]$TarpitHoldSecs = 300,
    [bool]$DenyPrivateIps = $true,
    [string[]]$AllowHosts = @(),
    [string[]]$BlockHosts = @("169.254.169.254", "metadata.google.internal"),
    [int[]]$AllowPorts = @(80, 443),
    [int[]]$BlockPorts = @(25),
    [ValidateSet("compact", "pretty", "json")]
    [string]$LogFormat = "compact",
    [string]$LogLevel = "info",
    [switch]$NoStart,
    [switch]$PrintCommand
)

$ErrorActionPreference = "Stop"

function New-Secret {
    $bytes = [byte[]]::new(32)
    [System.Security.Cryptography.RandomNumberGenerator]::Fill($bytes)
    [Convert]::ToBase64String($bytes)
}

function Quote-Toml {
    param([string]$Value)
    '"' + $Value.Replace('\', '\\').Replace('"', '\"') + '"'
}

function String-Array {
    param([string[]]$Values)
    if (!$Values -or $Values.Count -eq 0) {
        return "[]"
    }
    "[" + (($Values | ForEach-Object { Quote-Toml $_ }) -join ", ") + "]"
}

function Number-Array {
    param([int[]]$Values)
    if (!$Values -or $Values.Count -eq 0) {
        return "[]"
    }
    "[" + (($Values | ForEach-Object { $_.ToString() }) -join ", ") + "]"
}

function Decode-ProfileUrl {
    param([string]$Value)
    $prefix = "espejismo://import/"
    if (!$Value.StartsWith($prefix)) {
        throw "-ProfileUrl must start with espejismo://import/"
    }
    $encoded = $Value.Substring($prefix.Length).Replace('-', '+').Replace('_', '/')
    switch ($encoded.Length % 4) {
        2 { $encoded += "==" }
        3 { $encoded += "=" }
        1 { throw "invalid profile URL base64 length" }
    }
    $json = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($encoded))
    $json | ConvertFrom-Json
}

if ($InstallDir -eq "") {
    $InstallDir = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
}
$InstallDir = [System.IO.Path]::GetFullPath($InstallDir)

if ($ConfigPath -eq "") {
    $ConfigPath = Join-Path $InstallDir "configs\espejismo-$Mode.toml"
}
$ConfigDir = Split-Path -Parent $ConfigPath
New-Item -ItemType Directory -Force $ConfigDir | Out-Null

if ($ProfileUrl -ne "") {
    if ($Mode -ne "local") {
        throw "-ProfileUrl is only supported in local mode"
    }
    $profile = Decode-ProfileUrl $ProfileUrl
    $Server = [string]$profile.server
    $Psk = [string]$profile.psk
    if ($profile.socks5_listen) {
        $Socks5Listen = [string]$profile.socks5_listen
    }
    if ($profile.http_listen) {
        $HttpListen = [string]$profile.http_listen
    }
    if ($profile.auth) {
        $ProxyUsername = [string]$profile.auth.username
        $ProxyPassword = [string]$profile.auth.password
    }
}

if ($Psk -eq "") {
    $Psk = New-Secret
}
if ($Mode -eq "local" -and $Server -eq "") {
    throw "-Server is required for local mode, for example -Server 203.0.113.10:6690"
}
if ($ProxyUsername -eq "" -and $ProxyPassword -ne "") {
    throw "-ProxyUsername is required when -ProxyPassword is set"
}
if ($ProxyUsername -ne "" -and $ProxyPassword -eq "") {
    $ProxyPassword = New-Secret
}
if ($AdminListen -ne "" -and $AdminToken -eq "") {
    $AdminToken = New-Secret
}

$binary = if ($Mode -eq "local") { "espejismo-local.exe" } else { "espejismo-remote.exe" }
$binaryPath = Join-Path $InstallDir "bin\$binary"
if (!(Test-Path $binaryPath)) {
    $binaryPath = Join-Path $InstallDir $binary
}
if (!(Test-Path $binaryPath)) {
    throw "could not find $binary under $InstallDir or $InstallDir\bin"
}

$denyPrivate = if ($DenyPrivateIps) { "true" } else { "false" }
$config = @"
[shared]
psk = $(Quote-Toml $Psk)
clock_skew_secs = $ClockSkewSecs
puzzle_bits = $PuzzleBits
max_padding = $MaxPadding
jitter_ms = $JitterMs
padding_chance_percent = $PaddingChancePercent
backpressure_threshold_ms = $BackpressureThresholdMs
backpressure_cooldown_ms = $BackpressureCooldownMs
tunnel_buffer = $TunnelBuffer
idle_timeout_secs = $IdleTimeoutSecs
max_streams = $MaxStreams

[shared.obfuscation]
profile = $(Quote-Toml $ObfuscationProfile)
randomize_chunks = $($RandomizeChunks.ToString().ToLowerInvariant())
min_chunk = $MinChunk
max_chunk = $MaxChunk

[logging]
level = $(Quote-Toml $LogLevel)
format = $(Quote-Toml $LogFormat)
ansi = true

"@

if ($AdminListen -ne "") {
    $config += @"
[admin]
listen = $(Quote-Toml $AdminListen)
token = $(Quote-Toml $AdminToken)

"@
}

if ($Mode -eq "local") {
    $config += @"
[local]
server = $(Quote-Toml $Server)
socks5_listen = $(Quote-Toml $Socks5Listen)
http_listen = $(Quote-Toml $HttpListen)
handshake_padding = $HandshakePadding

"@
    if ($ProxyUsername -ne "") {
        $config += @"
[local.auth]
username = $(Quote-Toml $ProxyUsername)
password = $(Quote-Toml $ProxyPassword)

"@
    }
} else {
    $config += @"
[remote]
listen = $(Quote-Toml $RemoteListen)
handshake_timeout_ms = $HandshakeTimeoutMs
reject_delay_ms = $RejectDelayMs
max_handshake_padding = $MaxHandshakePadding
replay_window_secs = $ReplayWindowSecs
cold_start_delay_ms = $ColdStartDelayMs
tarpit_max = $TarpitMax
tarpit_hold_secs = $TarpitHoldSecs

[remote.egress]
deny_private_ips = $denyPrivate
allow_hosts = $(String-Array $AllowHosts)
block_hosts = $(String-Array $BlockHosts)
allow_ports = $(Number-Array $AllowPorts)
block_ports = $(Number-Array $BlockPorts)

"@
}

Set-Content -LiteralPath $ConfigPath -Value $config -Encoding UTF8

$commandLine = "& `"$binaryPath`" --config `"$ConfigPath`""
Write-Host "Wrote config: $ConfigPath"
Write-Host "Binary: $binaryPath"
if ($Mode -eq "local") {
    Write-Host "SOCKS5: $Socks5Listen"
    Write-Host "HTTP proxy: $HttpListen"
}
if ($PrintCommand -or $NoStart) {
    Write-Host "Run:"
    Write-Host "  $commandLine"
}
if (!$NoStart) {
    & $binaryPath --config $ConfigPath
}
