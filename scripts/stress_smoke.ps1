param(
    [int]$Requests = 200,
    [int]$Concurrency = 16,
    [string]$Cargo = ""
)

$ErrorActionPreference = "Stop"

$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Python = (Get-Command python -ErrorAction Stop).Source
$Curl = (Get-Command curl.exe -ErrorAction Stop).Source
if ($Cargo -eq "") {
    $CargoCommand = Get-Command cargo -ErrorAction SilentlyContinue
    if ($CargoCommand) {
        $Cargo = $CargoCommand.Source
    } else {
        $Cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"
    }
}
if (!(Test-Path $Cargo)) {
    throw "cargo was not found; pass -Cargo or add it to PATH"
}

$PortBase = 24000 + ($PID % 20000)
$HttpAddr = "127.0.0.1"
$HttpPort = $PortBase
$RemoteAddr = "127.0.0.1:$($PortBase + 1)"
$Socks5Addr = "127.0.0.1:$($PortBase + 2)"
$HttpProxyAddr = "127.0.0.1:$($PortBase + 3)"
$ConfigFile = Join-Path ([System.IO.Path]::GetTempPath()) "espejismo-stress-$PID.toml"
$ProbeToken = "stress-$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())-$PID"
$MuxMode = if ($env:MUX_MODE) { $env:MUX_MODE } else { "yamux" }
$Processes = @()

function Start-ProbeProcess {
    param(
        [string]$FilePath,
        [string[]]$Arguments,
        [string]$Name
    )

    $stdout = Join-Path ([System.IO.Path]::GetTempPath()) "espejismo-$Name-$PID.out.log"
    $stderr = Join-Path ([System.IO.Path]::GetTempPath()) "espejismo-$Name-$PID.err.log"
    $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -WorkingDirectory $Root -RedirectStandardOutput $stdout -RedirectStandardError $stderr -WindowStyle Hidden -PassThru
    $script:Processes += $process
    return $process
}

function Wait-Port {
    param(
        [string]$HostName,
        [int]$Port,
        [string]$Name
    )

    for ($i = 0; $i -lt 80; $i++) {
        $client = [System.Net.Sockets.TcpClient]::new()
        try {
            $connect = $client.BeginConnect($HostName, $Port, $null, $null)
            if ($connect.AsyncWaitHandle.WaitOne(250)) {
                $client.EndConnect($connect)
                return
            }
        } catch {
        } finally {
            $client.Close()
        }
        Start-Sleep -Milliseconds 250
    }
    throw "timed out waiting for $Name at ${HostName}:$Port"
}

try {
    Set-Location $Root

    Start-ProbeProcess $Python @("$Root\scripts\probe_http_server.py", "--host", $HttpAddr, "--port", "$HttpPort") "stress-http" | Out-Null
    Wait-Port $HttpAddr $HttpPort "http fixture"

    @"
[shared]
psk = "stress-secret-that-is-long-enough"
puzzle_bits = 8
max_streams = 32
idle_timeout_secs = 60

[shared.mux]
mode = "$MuxMode"
native_initial_window_bytes = 1048576
native_stream_buffer_frames = 128
native_idle_timeout_secs = 60

[shared.obfuscation]
profile = "high_entropy"
randomize_chunks = true
min_chunk = 256
max_chunk = 4096

[local]
server = "$RemoteAddr"
socks5_listen = "$Socks5Addr"
http_listen = "$HttpProxyAddr"
handshake_padding = 512

[local.tunnel_pool]
min_connections = 1
max_connections = 4
interactive_lanes = 1
bulk_lanes = 2
max_reconnect_attempts = 3

[logging]
level = "info"
format = "compact"
ansi = false

[remote]
listen = "$RemoteAddr"
handshake_timeout_ms = 1000
reject_delay_ms = 0
cold_start_delay_ms = 0

[remote.egress]
allow_ports = [$HttpPort]
"@ | Set-Content -LiteralPath $ConfigFile -Encoding UTF8

    Start-ProbeProcess $Cargo @("run", "--quiet", "--bin", "espejismo-remote", "--", "--config", $ConfigFile) "stress-remote" | Out-Null
    Wait-Port "127.0.0.1" ($PortBase + 1) "remote"

    Start-ProbeProcess $Cargo @("run", "--quiet", "--bin", "espejismo-local", "--", "--config", $ConfigFile) "stress-local" | Out-Null
    Wait-Port "127.0.0.1" ($PortBase + 2) "SOCKS5 proxy"
    Wait-Port "127.0.0.1" ($PortBase + 3) "HTTP proxy"

    $jobs = @()
    for ($idx = 0; $idx -lt $Requests; $idx++) {
        while (($jobs | Where-Object { $_.State -eq "Running" }).Count -ge $Concurrency) {
            $finished = Wait-Job -Job $jobs -Any -Timeout 1
            if ($finished) {
                Receive-Job -Job $finished | Out-Null
                if ($finished.State -ne "Completed") {
                    throw "stress job failed"
                }
                Remove-Job -Job $finished
                $jobs = @($jobs | Where-Object { $_.Id -ne $finished.Id })
            }
        }

        $path = "/stress/$idx/$ProbeToken"
        $useSocks = ($idx % 2 -eq 0)
        $jobs += Start-Job -ScriptBlock {
            param($Curl, $UseSocks, $Socks5Addr, $HttpProxyAddr, $HttpAddr, $HttpPort, $Path, $ProbeToken)
            if ($UseSocks) {
                $output = & $Curl --silent --show-error --max-time 10 --socks5-hostname $Socks5Addr -H "X-Espejismo-Probe: $ProbeToken" "http://${HttpAddr}:${HttpPort}${Path}"
            } else {
                $output = & $Curl --silent --show-error --max-time 10 --proxy "http://$HttpProxyAddr" -H "X-Espejismo-Probe: $ProbeToken" "http://${HttpAddr}:${HttpPort}${Path}"
            }
            if ($LASTEXITCODE -ne 0 -or !(($output -join "`n").Contains("`"probe`": `"$ProbeToken`""))) {
                throw "probe failed for $Path"
            }
        } -ArgumentList $Curl, $useSocks, $Socks5Addr, $HttpProxyAddr, $HttpAddr, $HttpPort, $path, $ProbeToken
    }

    while ($jobs.Count -gt 0) {
        $finished = Wait-Job -Job $jobs -Any
        Receive-Job -Job $finished | Out-Null
        if ($finished.State -ne "Completed") {
            throw "stress job failed"
        }
        Remove-Job -Job $finished
        $jobs = @($jobs | Where-Object { $_.Id -ne $finished.Id })
    }

    Write-Host "stress smoke passed: $Requests requests at concurrency $Concurrency"
} finally {
    foreach ($process in $Processes) {
        if ($process -and !$process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -LiteralPath $ConfigFile -Force -ErrorAction SilentlyContinue
}
