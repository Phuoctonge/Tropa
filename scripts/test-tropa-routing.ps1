#requires -Version 7.2
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('baseline', 'connected', 'after')][string]$Phase,
    [Parameter(Mandatory)][string]$Fixture,
    [Parameter(Mandatory)][string]$Target,
    [ValidatePattern('^[a-zA-Z0-9-]{1,64}$')][string]$RunId = 'manual',
    [switch]$IncludeLongUdp
)

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/tropa-routing-phase.ps1"
$repoRoot = Split-Path -Parent $PSScriptRoot
$runRoot = Join-Path $repoRoot ".artifacts/validation/routing/$RunId"
$fixtureSource = (Resolve-Path -LiteralPath $Fixture).Path
$parsedTarget = $null
if (-not [System.Net.IPEndPoint]::TryParse($Target, [ref]$parsedTarget)) {
    throw 'Target must be an IP socket address for a controlled echo responder.'
}
if ($parsedTarget.Port -eq 0 -or $parsedTarget.Port -eq 53 -or
    [System.Net.IPAddress]::IsLoopback($parsedTarget.Address) -or
    $parsedTarget.Address.Equals([System.Net.IPAddress]::Any) -or
    $parsedTarget.Address.Equals([System.Net.IPAddress]::IPv6Any)) {
    throw 'Native routing checks require a remote, non-DNS echo responder; loopback cannot prove routing.'
}
$reportPath = Join-Path $runRoot "$Phase.json"
if (Test-Path -LiteralPath $reportPath) { throw 'This phase already has a report; use a new RunId.' }
$fixtureHash = (Get-FileHash -LiteralPath $fixtureSource -Algorithm SHA256).Hash
$targetBytes = [Text.Encoding]::UTF8.GetBytes($parsedTarget.ToString())
$targetHash = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($targetBytes))
Assert-TropaRoutingPhase -Phase $Phase -RunRoot $runRoot -TargetHash $targetHash -FixtureHash $fixtureHash -LongUdp ([bool]$IncludeLongUdp)
$paths = @{}
foreach ($role in @('selected', 'direct')) {
    $folder = Join-Path $runRoot $role
    $null = New-Item -ItemType Directory -Path $folder -Force
    $destination = Join-Path $folder 'tropa-routing-fixture.exe'
    if ($Phase -eq 'baseline') {
        if (Test-Path -LiteralPath $destination) { throw 'Fixture already exists; use a new RunId.' }
        Copy-Item -LiteralPath $fixtureSource -Destination $destination
    }
    if ((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $fixtureHash) {
        throw 'Fixture differs between process paths or phases.'
    }
    $paths[$role] = $destination
}

function Get-StateFingerprint {
    $snapshot = [ordered]@{
        routes = @(Get-NetRoute | Select-Object AddressFamily, DestinationPrefix, NextHop, InterfaceIndex, RouteMetric |
            Sort-Object AddressFamily, DestinationPrefix, NextHop, InterfaceIndex)
        dns = @(Get-DnsClientServerAddress | Select-Object InterfaceIndex, AddressFamily, ServerAddresses |
            Sort-Object InterfaceIndex, AddressFamily)
        adapters = @(Get-NetAdapter -IncludeHidden | Select-Object Name, InterfaceGuid, Status |
            Sort-Object InterfaceGuid)
        proxy = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' |
            Select-Object ProxyEnable, ProxyServer, ProxyOverride, AutoConfigURL
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes(($snapshot | ConvertTo-Json -Depth 8 -Compress))
    $hash = [Security.Cryptography.SHA256]::Create()
    try { return [Convert]::ToHexString($hash.ComputeHash($bytes)) }
    finally { $hash.Dispose() }
}

$before = Get-StateFingerprint
$cases = @(
    @{ name = 'tcp-short'; transport = 'tcp'; mode = 'fresh'; samples = 20; interval = 0 },
    @{ name = 'udp-burst'; transport = 'udp'; mode = 'persistent'; samples = 100; interval = 0 },
    @{ name = 'udp-short'; transport = 'udp'; mode = 'fresh'; samples = 20; interval = 0 }
)
if ($IncludeLongUdp) {
    $cases += @{ name = 'udp-five-minutes'; transport = 'udp'; mode = 'persistent'; samples = 301; interval = 1000 }
}
$results = @()
foreach ($case in $cases) {
    foreach ($role in @('selected', 'direct')) {
        $nonce = [Guid]::NewGuid().ToString('N')
        $fixtureArgs = @('--transport', $case.transport, '--target', $Target, '--nonce', $nonce,
            '--samples', [string]$case.samples, '--interval-ms', [string]$case.interval,
            '--socket-mode', $case.mode, '--timeout-ms', '1000')
        $raw = & $paths[$role] @fixtureArgs 2>$null
        $exitCode = $LASTEXITCODE
        try { $measurement = ($raw -join "`n") | ConvertFrom-Json }
        catch { throw 'Fixture did not return a valid public measurement.' }
        $results += [ordered]@{ case = $case.name; role = $role; exitCode = $exitCode; measurement = $measurement }
    }
}
$after = Get-StateFingerprint
$report = [ordered]@{
    schemaVersion = 2
    targetSha256 = $targetHash
    phase = $Phase
    fixtureSha256 = $fixtureHash
    targetFamily = [string]$parsedTarget.AddressFamily
    selectedFixture = $paths.selected
    directFixture = $paths.direct
    beforeStateSha256 = $before
    afterStateSha256 = $after
    stateUnchangedDuringProbes = $before -eq $after
    longUdpTested = [bool]$IncludeLongUdp
    nativeRoutingVerified = $false
    requiresReview = $true
    results = $results
}
$report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $reportPath -Encoding utf8
Write-Output $reportPath
