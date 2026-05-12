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
    [bool]$TcpNoDelay = $true,
    [int]$TcpKeepaliveSecs = 30,
    [int]$TcpHeartbeatSecs = 30,
    [int]$TcpUserTimeoutMs = 30000,
    [int]$TcpSendBufferBytes = 1048576,
    [int]$TcpRecvBufferBytes = 1048576,
    [ValidateSet("yamux", "native")]
    [string]$MuxMode = "yamux",
    [int]$NativeMuxInitialWindowBytes = 1048576,
    [int]$NativeMuxStreamBufferFrames = 128,
    [int]$NativeMuxSendQueueFrames = 64,
    [int]$NativeMuxIdleTimeoutSecs = 300,
    [int]$NativeMuxDrainTimeoutSecs = 30,
    [bool]$PacingEnabled = $true,
    [int64]$PacingMaxBytesPerSec = 0,
    [int]$PacingBurstBytes = 65536,
    [int]$PacingMinWriteBytes = 1024,
    [ValidateSet("low_latency", "balanced", "high_entropy", "bulk", "stealth")]
    [string]$ObfuscationProfile = "balanced",
    [ValidateSet("low_latency", "balanced", "bulk", "stealth", "custom")]
    [string]$ChunkPolicy = "balanced",
    [bool]$RandomizeChunks = $true,
    [int]$MinChunk = 4096,
    [int]$MaxChunk = 16384,
    [int]$StealthFrameSize = 4096,
    [int]$StealthTickMs = 50,
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
    [int]$TunnelPoolMinConnections = 1,
    [int]$TunnelPoolMaxConnections = 4,
    [int]$TunnelPoolInteractiveLanes = 1,
    [int]$TunnelPoolBulkLanes = 2,
    [int]$TunnelPoolMaxReconnectAttempts = 3,
    [ValidateSet("compact", "pretty", "json")]
    [string]$LogFormat = "compact",
    [string]$LogLevel = "info",
    [switch]$NoStart,
    [switch]$PrintCommand,
    [switch]$TunEnabled,
    [string]$TunName = "esptun0",
    [string]$TunAddress = "10.255.0.2",
    [int]$TunPrefix = 24,
    [string]$TunDestination = "10.255.0.1",
    [int]$TunMtu = 1500,
    [switch]$TunAutoRoute,
    [switch]$TunAutoDns,
    [string]$TunDns = "1.1.1.1,8.8.8.8"
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

function Assert-WintunRuntime {
    param(
        [string]$BinaryPath,
        [string]$InstallDir
    )
    $binaryDir = Split-Path -Parent $BinaryPath
    $candidates = @(
        (Join-Path $binaryDir "wintun.dll"),
        (Join-Path $InstallDir "bin\wintun.dll"),
        (Join-Path $InstallDir "wintun.dll")
    )
    foreach ($candidate in $candidates) {
        if (Test-Path $candidate) {
            return
        }
    }
    throw @"
TUN mode requires wintun.dll, but it was not found.
Expected one of:
  $($candidates -join "`n  ")

Fix:
  1) Put wintun.dll next to espejismo-local.exe (recommended).
  2) Re-run:
     .\bin\espejismo-local.exe --config <config> --tun-enabled --tun-auto-route --tun-auto-dns
"@
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
if ($Mode -ne "local" -and ($TunEnabled -or $TunAutoRoute -or $TunAutoDns)) {
    throw "TUN flags are only supported in local mode"
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

[shared.tcp]
nodelay = $($TcpNoDelay.ToString().ToLowerInvariant())
keepalive_secs = $TcpKeepaliveSecs
heartbeat_secs = $TcpHeartbeatSecs
user_timeout_ms = $TcpUserTimeoutMs
send_buffer_bytes = $TcpSendBufferBytes
recv_buffer_bytes = $TcpRecvBufferBytes

[shared.mux]
mode = $(Quote-Toml $MuxMode)
native_initial_window_bytes = $NativeMuxInitialWindowBytes
native_stream_buffer_frames = $NativeMuxStreamBufferFrames
native_send_queue_frames = $NativeMuxSendQueueFrames
native_idle_timeout_secs = $NativeMuxIdleTimeoutSecs
native_drain_timeout_secs = $NativeMuxDrainTimeoutSecs

[shared.pacing]
enabled = $($PacingEnabled.ToString().ToLowerInvariant())
max_bytes_per_sec = $PacingMaxBytesPerSec
burst_bytes = $PacingBurstBytes
min_write_bytes = $PacingMinWriteBytes

[shared.obfuscation]
profile = $(Quote-Toml $ObfuscationProfile)
chunk_policy = $(Quote-Toml $ChunkPolicy)
randomize_chunks = $($RandomizeChunks.ToString().ToLowerInvariant())
min_chunk = $MinChunk
max_chunk = $MaxChunk

[shared.stealth]
frame_size = $StealthFrameSize
tick_ms = $StealthTickMs

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

[local.tunnel_pool]
min_connections = $TunnelPoolMinConnections
max_connections = $TunnelPoolMaxConnections
interactive_lanes = $TunnelPoolInteractiveLanes
bulk_lanes = $TunnelPoolBulkLanes
max_reconnect_attempts = $TunnelPoolMaxReconnectAttempts

[local.tun]
enabled = $($TunEnabled.ToString().ToLowerInvariant())
name = $(Quote-Toml $TunName)
address = $(Quote-Toml $TunAddress)
prefix = $TunPrefix
destination = $(Quote-Toml $TunDestination)
mtu = $TunMtu

[local.tun.route]
enabled = $($TunAutoRoute.ToString().ToLowerInvariant())
protect_server_route = true
dns_enabled = $($TunAutoDns.ToString().ToLowerInvariant())
dns_servers = $(String-Array ($TunDns -split "," | ForEach-Object { $_.Trim() } | Where-Object { $_ -ne "" }))

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
$startArgs = @("--config", $ConfigPath)
if ($Mode -eq "local") {
    if ($TunEnabled) {
        $commandLine += " --tun-enabled"
        $startArgs += "--tun-enabled"
    }
    if ($TunAutoRoute) {
        $commandLine += " --tun-auto-route"
        $startArgs += "--tun-auto-route"
    }
    if ($TunAutoDns) {
        $commandLine += " --tun-auto-dns"
        $startArgs += "--tun-auto-dns"
    }
    if ($TunDns -ne "") {
        $commandLine += " --tun-dns `"$TunDns`""
        $startArgs += @("--tun-dns", $TunDns)
    }
}
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
    if ($Mode -eq "local" -and $TunEnabled) {
        Assert-WintunRuntime -BinaryPath $binaryPath -InstallDir $InstallDir
    }
    & $binaryPath @startArgs
}
