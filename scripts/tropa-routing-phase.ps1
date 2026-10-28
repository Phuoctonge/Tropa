function Assert-TropaRoutingPhase {
    param(
        [Parameter(Mandatory)][ValidateSet('baseline', 'connected', 'after')][string]$Phase,
        [Parameter(Mandatory)][string]$RunRoot,
        [Parameter(Mandatory)][string]$TargetHash,
        [Parameter(Mandatory)][string]$FixtureHash,
        [Parameter(Mandatory)][bool]$LongUdp
    )
    $required = switch ($Phase) {
        'baseline' { @() }
        'connected' { @('baseline') }
        'after' { @('baseline', 'connected') }
    }
    foreach ($name in $required) {
        $path = Join-Path $RunRoot "$name.json"
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Missing $name report; execute phases in baseline, connected, after order."
        }
        $report = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        if ($report.schemaVersion -ne 2 -or $report.phase -ne $name -or
            $report.targetSha256 -ne $TargetHash -or $report.fixtureSha256 -ne $FixtureHash -or
            $report.longUdpTested -isnot [bool] -or $report.longUdpTested -ne $LongUdp) {
            throw 'Phase reports must use the same target, fixture and long-UDP setting; use a new RunId for a different test.'
        }
    }
}
