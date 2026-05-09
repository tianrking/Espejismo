param(
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

$PortBase = 20000 + ($PID % 20000)
$HttpAddr = "127.0.0.1"
$HttpPort = $PortBase
$RemoteAddr = "127.0.0.1:$($PortBase + 1)"
$Socks5Addr = "127.0.0.1:$($PortBase + 2)"
$HttpProxyAddr = "127.0.0.1:$($PortBase + 3)"
$LocalAdminAddr = "127.0.0.1:$($PortBase + 4)"
$RemoteAdminAddr = "127.0.0.1:$($PortBase + 5)"
$UdpPort = $PortBase + 6
$ConfigFile = Join-Path ([System.IO.Path]::GetTempPath()) "espejismo-config-$PID.toml"
$ProbeToken = "probe-$([DateTimeOffset]::UtcNow.ToUnixTimeSeconds())-$PID"
$PostBody = "body-$ProbeToken"
$ProxyUser = "probe-user"
$ProxyPass = "probe-pass"
$AdminToken = "admin-$ProbeToken"
$Psk = "change-me-long-random-secret"
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

function Invoke-CurlText {
    param([string[]]$Arguments)

    $output = & $Curl @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "curl failed with exit code $LASTEXITCODE"
    }
    return ($output -join "`n")
}

function Assert-Contains {
    param(
        [string]$Text,
        [string]$Needle
    )

    if (!$Text.Contains($Needle)) {
        throw "expected output to contain '$Needle', got: $Text"
    }
}

try {
    Set-Location $Root

    Start-ProbeProcess $Python @("$Root\scripts\probe_http_server.py", "--host", $HttpAddr, "--port", "$HttpPort") "http" | Out-Null
    Wait-Port $HttpAddr $HttpPort "http fixture"

    Start-ProbeProcess $Python @("$Root\scripts\probe_udp_server.py", "--host", $HttpAddr, "--port", "$UdpPort") "udp" | Out-Null

    @"
[shared]
psk = "$Psk"
puzzle_bits = 12
max_streams = 2

[local]
server = "$RemoteAddr"
socks5_listen = "$Socks5Addr"
http_listen = "$HttpProxyAddr"
handshake_padding = 256

[local.auth]
username = "$ProxyUser"
password = "$ProxyPass"

[logging]
level = "debug"
format = "json"
ansi = false

[admin]
listen = "$LocalAdminAddr"
token = "$AdminToken"

[remote]
listen = "$RemoteAddr"
handshake_timeout_ms = 1000
reject_delay_ms = 25
cold_start_delay_ms = 20

[remote.egress]
allow_ports = [$HttpPort, $UdpPort]
"@ | Set-Content -LiteralPath $ConfigFile -Encoding UTF8

    $ConfigB64 = [Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes((Get-Content -LiteralPath $ConfigFile -Raw)))
    $ProfileUrl = & $Cargo run --quiet --bin espejismo-local -- --config $ConfigFile --print-client-profile --profile-name smoke
    if ($LASTEXITCODE -ne 0 -or !$ProfileUrl.StartsWith("espejismo://import/")) {
        throw "unexpected profile URL: $ProfileUrl"
    }
    & $Cargo run --quiet --bin espejismo-local -- --import-profile $ProfileUrl --print-client-profile --profile-name smoke-imported | Out-String | ForEach-Object {
        Assert-Contains $_ "espejismo://import/"
    }

    Start-ProbeProcess $Cargo @("run", "--quiet", "--bin", "espejismo-remote", "--", "--config-base64", $ConfigB64, "--admin-listen", $RemoteAdminAddr) "remote" | Out-Null
    Wait-Port "127.0.0.1" ($PortBase + 1) "espejismo remote"
    Wait-Port "127.0.0.1" ($PortBase + 5) "espejismo remote admin"

    Start-ProbeProcess $Cargo @("run", "--quiet", "--bin", "espejismo-local", "--", "--config", $ConfigFile) "local" | Out-Null
    Wait-Port "127.0.0.1" ($PortBase + 2) "SOCKS5 proxy"
    Wait-Port "127.0.0.1" ($PortBase + 3) "HTTP proxy"
    Wait-Port "127.0.0.1" ($PortBase + 4) "espejismo local admin"

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "10", "--proxy-user", "${ProxyUser}:${ProxyPass}", "--socks5-hostname", $Socks5Addr, "-H", "X-Espejismo-Probe: $ProbeToken", "http://${HttpAddr}:${HttpPort}/probe/socks5/$ProbeToken")
    Assert-Contains $text "`"probe`": `"$ProbeToken`""

    foreach ($idx in 1..4) {
        $seqToken = "$ProbeToken-seq-$idx"
        $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "10", "--proxy-user", "${ProxyUser}:${ProxyPass}", "--socks5-hostname", $Socks5Addr, "-H", "X-Espejismo-Probe: $seqToken", "http://${HttpAddr}:${HttpPort}/probe/sequential/$idx/$ProbeToken")
        Assert-Contains $text "`"path`": `"/probe/sequential/$idx/$ProbeToken`""
    }

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "10", "--proxy-user", "${ProxyUser}:${ProxyPass}", "--socks5-hostname", $Socks5Addr, "-X", "POST", "-H", "X-Espejismo-Probe: $ProbeToken", "--data", $PostBody, "http://${HttpAddr}:${HttpPort}/probe/post/$ProbeToken")
    Assert-Contains $text "`"body`": `"$PostBody`""

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "10", "--proxy", "http://$HttpProxyAddr", "--proxy-user", "${ProxyUser}:${ProxyPass}", "-H", "X-Espejismo-Probe: $ProbeToken", "http://${HttpAddr}:${HttpPort}/probe/http/$ProbeToken")
    Assert-Contains $text "`"path`": `"/probe/http/$ProbeToken`""

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "10", "--proxytunnel", "--proxy", "http://$HttpProxyAddr", "--proxy-user", "${ProxyUser}:${ProxyPass}", "-H", "X-Espejismo-Probe: $ProbeToken", "http://${HttpAddr}:${HttpPort}/probe/connect/$ProbeToken")
    Assert-Contains $text "`"path`": `"/probe/connect/$ProbeToken`""

    $udp = & $Python "$Root\scripts\probe_socks5_udp.py" --socks-port "$($PortBase + 2)" --username $ProxyUser --password $ProxyPass --target-port "$UdpPort" --payload $ProbeToken
    if ($LASTEXITCODE -ne 0) {
        throw "SOCKS5 UDP probe failed"
    }
    Assert-Contains ($udp -join "`n") "udp-echo:$ProbeToken"

    $authStatus = & $Curl --silent --output NUL --write-out "%{http_code}" --max-time 5 --proxy "http://$HttpProxyAddr" "http://${HttpAddr}:${HttpPort}/probe/reject/$ProbeToken"
    if (($authStatus -join "") -ne "407") {
        throw "expected HTTP auth rejection 407, got $authStatus"
    }

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "5", "-H", "Authorization: Bearer $AdminToken", "http://$LocalAdminAddr/healthz")
    Assert-Contains $text "ok"

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "5", "-H", "Authorization: Bearer $AdminToken", "http://$LocalAdminAddr/status")
    Assert-Contains $text "`"role`": `"local`""

    $text = Invoke-CurlText @("--silent", "--show-error", "--max-time", "5", "-H", "Authorization: Bearer $AdminToken", "http://$RemoteAdminAddr/metrics")
    Assert-Contains $text "espejismo_stream_opened_total"

    $adminStatus = & $Curl --silent --output NUL --write-out "%{http_code}" --max-time 5 "http://$LocalAdminAddr/status"
    if (($adminStatus -join "") -ne "401") {
        throw "expected admin auth rejection 401, got $adminStatus"
    }

    Write-Host "e2e smoke test passed"
} finally {
    foreach ($process in $Processes) {
        if ($process -and !$process.HasExited) {
            Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        }
    }
    Remove-Item -LiteralPath $ConfigFile -Force -ErrorAction SilentlyContinue
}
