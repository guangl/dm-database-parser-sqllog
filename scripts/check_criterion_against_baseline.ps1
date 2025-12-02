$ErrorActionPreference = 'Stop'

param(
    [string]$Baseline = 'benchmarks/baseline.json',
    [double]$TolerancePercent = 0
)

if (!(Test-Path $Baseline)) {
    Write-Error "Baseline file not found: $Baseline. Run scripts/export_criterion_baseline.ps1 first."
}

Write-Host 'Running cargo bench to get current Criterion results...'
cargo bench --bench parser_benchmark | Out-Null

$baselineMap = Get-Content -Raw -Path $Baseline | ConvertFrom-Json
$root = Join-Path (Get-Location) 'target/criterion'
$estimates = Get-ChildItem -Path $root -Recurse -Filter estimates.json | Where-Object { $_.FullName -match "\\new\\estimates.json$|/new/estimates.json$" }

if (-not $estimates) {
    Write-Error 'No new/estimates.json found. Did benchmarks run?'
}

$violations = @()
foreach ($file in $estimates) {
    $json = Get-Content -Raw -Path $file.FullName | ConvertFrom-Json
    $benchDir = Split-Path -Parent $file.Directory.FullName
    $benchName = Split-Path -Parent $benchDir
    $groupName = Split-Path -Parent $benchName
    $benchLeaf = Split-Path -Leaf $benchName
    $groupLeaf = Split-Path -Leaf $groupName
    $key = "$groupLeaf/$benchLeaf"
    $current = [double]$json.mean.point_estimate
    $baseline = [double]$baselineMap.$key
    if ($null -eq $baseline) {
        Write-Warning "No baseline for $key; skipping."
        continue
    }
    $limit = $baseline * (1.0 + ($TolerancePercent / 100.0))
    if ($current -gt $limit) {
        $delta = (($current / $baseline) - 1.0) * 100.0
        $violations += [pscustomobject]@{ Name = $key; BaselineNs = [int64]$baseline; CurrentNs = [int64]$current; RegressionPct = [math]::Round($delta, 2) }
    }
}

if ($violations.Count -gt 0) {
    Write-Host "Benchmark regressions detected:" -ForegroundColor Red
    $violations | Format-Table -AutoSize | Out-String | Write-Host
    exit 1
} else {
    Write-Host 'Benchmarks meet or beat baseline.' -ForegroundColor Green
}
