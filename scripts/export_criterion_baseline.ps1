$ErrorActionPreference = 'Stop'

param(
    [string]$Output = 'benchmarks/baseline.json'
)

Write-Host 'Running cargo bench to produce Criterion outputs...'
cargo bench --bench parser_benchmark | Out-Null

$root = Join-Path (Get-Location) 'target/criterion'
if (!(Test-Path $root)) {
    Write-Error "Criterion output directory not found: $root"
}

$estimates = Get-ChildItem -Path $root -Recurse -Filter estimates.json | Where-Object { $_.FullName -match "\\new\\estimates.json$|/new/estimates.json$" }

if (-not $estimates) {
    Write-Error 'No new/estimates.json found. Did benchmarks run?'
}

$map = @{}
foreach ($file in $estimates) {
    $json = Get-Content -Raw -Path $file.FullName | ConvertFrom-Json
    $benchDir = Split-Path -Parent $file.Directory.FullName # .../parse_sqllog_file_xxx/new
    $benchName = Split-Path -Parent $benchDir               # .../parse_sqllog_file_xxx
    $groupName = Split-Path -Parent $benchName              # .../parser_group
    $benchLeaf = Split-Path -Leaf $benchName
    $groupLeaf = Split-Path -Leaf $groupName
    $key = "$groupLeaf/$benchLeaf"
    $map[$key] = [double]$json.mean.point_estimate
}

New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Output) | Out-Null
$map | ConvertTo-Json | Out-File -Encoding UTF8 -FilePath $Output
Write-Host "Baseline exported to $Output"
