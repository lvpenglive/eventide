# Seed demo data into a running Eventide instance.
# Usage: powershell -File scripts/seed-demo.ps1

$ErrorActionPreference = 'Stop'
$base = if ($env:EVENTIDE_URL) { $env:EVENTIDE_URL.TrimEnd('/') } else { 'http://127.0.0.1:8080' }
$root = Split-Path -Parent $PSScriptRoot
if (-not $PSScriptRoot) { $root = Get-Location }
$seedPath = Join-Path $PSScriptRoot 'seed-demo.json'
if (-not (Test-Path $seedPath)) {
  $seedPath = Join-Path (Get-Location) 'scripts/seed-demo.json'
}

$seed = Get-Content -Raw -Encoding UTF8 $seedPath | ConvertFrom-Json

$login = Invoke-RestMethod -Uri "$base/api/auth/login" -Method POST -ContentType 'application/json' -Body '{"username":"admin","password":"admin123"}'
$auth = @{ Authorization = "Bearer $($login.token)" }

function ApiDelete($path) {
  try {
    Invoke-RestMethod -Uri "$base$path" -Method DELETE -Headers $auth | Out-Null
  } catch {}
}

function ApiPost($path, $obj) {
  $json = $obj | ConvertTo-Json -Depth 12 -Compress
  $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
  return Invoke-RestMethod -Uri "$base$path" -Method POST -Headers (@{
    Authorization = $auth.Authorization
    'Content-Type' = 'application/json; charset=utf-8'
  }) -Body $bytes
}

function ApiGet($path) {
  return Invoke-RestMethod -Uri "$base$path" -Headers $auth
}

Write-Host "Clearing existing demo-ish data..."
foreach ($col in @('rules','ingress','enrich','silences','channels','datasources')) {
  $rows = ApiGet "/api/$col"
  foreach ($r in $rows) {
    ApiDelete "/api/$col/$($r.id)"
  }
}
# alerts have no delete API — leave old ones; new ingress tests will add more

Write-Host "Seeding datasources..."
$dsMap = @{}
foreach ($d in $seed.datasources) {
  $created = ApiPost '/api/datasources' @{
    name = $d.name; kind = $d.kind; url = $d.url
    options = $d.options; enabled = [bool]$d.enabled
  }
  $dsMap[$d.name] = $created.id
}

Write-Host "Seeding channels..."
$chMap = @{}
foreach ($c in $seed.channels) {
  $created = ApiPost '/api/channels' @{
    name = $c.name; kind = $c.kind; url = $c.url
    secret = $c.secret; enabled = [bool]$c.enabled
  }
  $chMap[$c.name] = $created.id
}

Write-Host "Seeding rules..."
foreach ($r in $seed.rules) {
  $chIds = @($r._channels | ForEach-Object { $chMap[$_] })
  ApiPost '/api/rules' @{
    name = $r.name
    datasource_id = $dsMap[$r._datasource]
    expr = $r.expr
    comparator = $r.comparator
    threshold = [double]$r.threshold
    for_seconds = [int]$r.for_seconds
    interval_seconds = [int]$r.interval_seconds
    severity = $r.severity
    labels = $r.labels
    annotations = $r.annotations
    channel_ids = $chIds
    enabled = [bool]$r.enabled
  } | Out-Null
}

Write-Host "Seeding ingress..."
$ingressIds = @()
$ingressMap = @{}
foreach ($i in $seed.ingress) {
  $chIds = @($i._channels | ForEach-Object { $chMap[$_] })
  $created = ApiPost '/api/ingress' @{
    name = $i.name; kind = $i.kind; token = $i.token
    endpoint = $i.endpoint; options = $i.options
    channel_ids = $chIds; enabled = [bool]$i.enabled
  }
  $ingressIds += $created.id
  $ingressMap[$i.name] = @{ id = $created.id; token = $i.token }
}

Write-Host "Seeding enrich..."
foreach ($e in $seed.enrich) {
  ApiPost '/api/enrich' @{
    name = $e.name; kind = $e.kind
    matchers = $e.matchers; match_key = $e.match_key
    templates = $e.templates; mappings = $e.mappings
    write_labels = [bool]$e.write_labels
    enabled = [bool]$e.enabled; priority = [int]$e.priority
  } | Out-Null
}

Write-Host "Seeding silence..."
$s = $seed.silence
$start = [DateTime]::UtcNow.AddHours([double]$s.hours_from).ToString('yyyy-MM-ddTHH:mm:ssZ')
$end = [DateTime]::UtcNow.AddHours([double]$s.hours_to).ToString('yyyy-MM-ddTHH:mm:ssZ')
ApiPost '/api/silences' @{
  comment = $s.comment; rule_id = $null
  matchers = $s.matchers; starts_at = $start; ends_at = $end
} | Out-Null

Write-Host "Pushing sample alerts via ingress test..."
foreach ($id in $ingressIds) {
  try {
    ApiPost "/api/ingress/$id/test" @{ scenario = 'firing' } | Out-Null
  } catch {
    Write-Host "  ingress test $id failed: $($_.Exception.Message)"
  }
}

if ($seed.sample_alerts) {
  Write-Host "Pushing custom sample alerts..."
  foreach ($a in $seed.sample_alerts) {
    $ingName = [string]$a._ingress
    $ing = $ingressMap[$ingName]
    if (-not $ing) {
      Write-Host "  skip sample: ingress '$ingName' not found"
      continue
    }
    # Prefer UTF-8 sample file when present (Windows PowerShell JSON encoding).
    $sampleFile = Join-Path $PSScriptRoot 'sample-zabbix-alert.json'
    if ((Test-Path $sampleFile) -and $ingName -eq 'Demo Zabbix Kafka') {
      $bytes = [System.IO.File]::ReadAllBytes($sampleFile)
      if ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF) {
        $bytes = $bytes[3..($bytes.Length - 1)]
      }
    } else {
      $payload = @{}
      $a.PSObject.Properties | ForEach-Object {
        if ($_.Name -ne '_ingress') { $payload[$_.Name] = $_.Value }
      }
      $json = $payload | ConvertTo-Json -Depth 12 -Compress
      $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
    }
    try {
      Invoke-RestMethod -Uri "$base/api/ingress/$($ing.id)/push" -Method POST -Headers (@{
        Authorization = "Bearer $($ing.token)"
        'Content-Type' = 'application/json; charset=utf-8'
      }) -Body $bytes | Out-Null
      Write-Host "  pushed sample via $ingName"
    } catch {
      Write-Host "  sample push failed ($ingName): $($_.Exception.Message)"
    }
  }
}

Write-Host ""
Write-Host "Done:"
foreach ($col in @('datasources','channels','rules','ingress','enrich','alerts','silences')) {
  $n = @(ApiGet "/api/$col").Count
  Write-Host ("  {0,-12} {1}" -f $col, $n)
}
