$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/tropa-routing-phase.ps1"
$runRoot = Join-Path ([IO.Path]::GetTempPath()) "tropa-phase-$([Guid]::NewGuid().ToString('N'))"
$null = New-Item -ItemType Directory -Path $runRoot
$passed = 0
function Expect-Rejected([scriptblock]$Action) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw 'Expected phase validation to reject inconsistent inputs.' }
    $script:passed++
}
function Write-Phase([string]$Name, [string]$Target = 'target', [string]$Fixture = 'fixture', [bool]$Long = $true) {
    @{ schemaVersion = 2; phase = $Name; targetSha256 = $Target; fixtureSha256 = $Fixture; longUdpTested = $Long } |
        ConvertTo-Json | Set-Content -LiteralPath (Join-Path $runRoot "$Name.json") -Encoding utf8
}
try {
    Assert-TropaRoutingPhase -Phase baseline -RunRoot $runRoot -TargetHash target -FixtureHash fixture -LongUdp $true
    $passed++
    Expect-Rejected { Assert-TropaRoutingPhase connected $runRoot target fixture $true }
    Write-Phase baseline
    Assert-TropaRoutingPhase connected $runRoot target fixture $true
    $passed++
    Expect-Rejected { Assert-TropaRoutingPhase connected $runRoot other fixture $true }
    Expect-Rejected { Assert-TropaRoutingPhase connected $runRoot target other $true }
    Expect-Rejected { Assert-TropaRoutingPhase connected $runRoot target fixture $false }
    Expect-Rejected { Assert-TropaRoutingPhase after $runRoot target fixture $true }
    Write-Phase connected
    Assert-TropaRoutingPhase after $runRoot target fixture $true
    $passed++
    @{ phase = 'baseline' } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $runRoot 'baseline.json')
    Expect-Rejected { Assert-TropaRoutingPhase connected $runRoot target fixture $true }
    Write-Output "$passed phase checks passed; no fixtures or network processes launched."
} finally {
    foreach ($name in @('baseline.json', 'connected.json')) {
        $path = Join-Path $runRoot $name
        if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path }
    }
    Remove-Item -LiteralPath $runRoot
}
