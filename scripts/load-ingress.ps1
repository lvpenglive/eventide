# Eventide ingress load / storm pressure test
# Pushes unique-fingerprint alerts via generic ingress (MeridianOps chain by default).
#
# Usage:
#   powershell -File scripts/load-ingress.ps1
#   powershell -File scripts/load-ingress.ps1 -Total 500 -Concurrency 40
#   powershell -File scripts/load-ingress.ps1 -Total 200 -Concurrency 20 -Notify:$false
#
# Env overrides: EVENTIDE_URL, LOAD_INGRESS_ID, LOAD_TOKEN, LOAD_TOTAL, LOAD_CONCURRENCY

param(
  [string]$BaseUrl = $(if ($env:EVENTIDE_URL) { $env:EVENTIDE_URL.TrimEnd('/') } else { 'http://127.0.0.1:8080' }),
  [string]$IngressId = $(if ($env:LOAD_INGRESS_ID) { $env:LOAD_INGRESS_ID } else { '45e66093-e100-4882-8fb7-b605f1a69766' }),
  [string]$Token = $(if ($env:LOAD_TOKEN) { $env:LOAD_TOKEN } else { 'test-ingress-token' }),
  [int]$Total = $(if ($env:LOAD_TOTAL) { [int]$env:LOAD_TOTAL } else { 300 }),
  [int]$Concurrency = $(if ($env:LOAD_CONCURRENCY) { [int]$env:LOAD_CONCURRENCY } else { 30 }),
  [switch]$Notify = $true,
  [int]$VerifySample = 20
)

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

if ($Concurrency -lt 1) { $Concurrency = 1 }
if ($Total -lt 1) { $Total = 1 }
if ($Concurrency -gt $Total) { $Concurrency = $Total }

$runId = Get-Date -Format 'yyyyMMdd-HHmmss'
$path = '/api/ingress/' + $IngressId + '/generic'
$uri = $BaseUrl + $path

Write-Host ''
Write-Host '=== Eventide Ingress Load ===' -ForegroundColor Cyan
Write-Host ('base={0}' -f $BaseUrl)
Write-Host ('ingress={0}  total={1}  concurrency={2}  run={3}' -f $IngressId, $Total, $Concurrency, $runId)
Write-Host ('url={0}' -f $uri)
Write-Host ''

# Warm health
try {
  $h = Invoke-WebRequest -Uri ($BaseUrl + '/api/health') -UseBasicParsing -TimeoutSec 5
  Write-Host ('health status={0}' -f $h.StatusCode) -ForegroundColor DarkGray
} catch {
  Write-Host ('health FAIL: {0}' -f $_.Exception.Message) -ForegroundColor Red
  exit 2
}

$script:ok = 0
$script:fail = 0
$script:r429 = 0
$script:accepted = 0
$script:latencies = New-Object System.Collections.Generic.List[double]
$lockObj = New-Object object

$pool = [RunspaceFactory]::CreateRunspacePool(1, $Concurrency)
$pool.Open()
$jobs = @()

$pushScript = {
  param($Uri, $Token, $RunId, $Index, $BaseUrl)

  $sw = [System.Diagnostics.Stopwatch]::StartNew()
  $fp = 'load-{0}-{1:D5}' -f $RunId, $Index
  $alertname = 'LoadStorm-{0}' -f ($Index % 10)
  $sev = @('warning', 'critical', 'info')[$Index % 3]
  $ip = '10.20.{0}.{1}' -f ([int](($Index / 256) % 256), ($Index % 256))
  $summary = 'load test {0} #{1} host={2}' -f $RunId, $Index, $ip
  # Generic ingress expects Alertmanager-like shape: labels/annotations.
  # Flat alertname/summary/ip fields are ignored when map_* is not configured.
  $body = @{
    status      = 'firing'
    fingerprint = $fp
    severity    = $sev
    labels      = @{
      alertname = $alertname
      severity  = $sev
      ip        = $ip
      instance  = $ip
      source    = 'load-ingress'
      run_id    = $RunId
    }
    annotations = @{
      summary     = $summary
      description = $summary
    }
  } | ConvertTo-Json -Depth 6 -Compress

  $result = [ordered]@{
    Index      = $Index
    Fp         = $fp
    StatusCode = 0
    Accepted   = 0
    Ms         = 0
    Error      = ''
  }

  try {
    $resp = Invoke-WebRequest -Uri $Uri -Method POST -Headers @{
      'X-Eventide-Token' = $Token
      'Content-Type'     = 'application/json'
    } -Body $body -UseBasicParsing -TimeoutSec 30
    $sw.Stop()
    $result.StatusCode = [int]$resp.StatusCode
    $result.Ms = $sw.Elapsed.TotalMilliseconds
    try {
      $j = $resp.Content | ConvertFrom-Json
      if ($null -ne $j.accepted) { $result.Accepted = [int]$j.accepted }
    } catch {}
  } catch {
    $sw.Stop()
    $result.Ms = $sw.Elapsed.TotalMilliseconds
    $ex = $_.Exception
    if ($ex.Response -and $ex.Response.StatusCode) {
      $result.StatusCode = [int]$ex.Response.StatusCode
      try {
        $reader = New-Object IO.StreamReader($ex.Response.GetResponseStream())
        $txt = $reader.ReadToEnd()
        $result.Error = $txt
      } catch {
        $result.Error = $ex.Message
      }
    } else {
      $result.Error = $ex.Message
    }
  }
  return [pscustomobject]$result
}

$swAll = [System.Diagnostics.Stopwatch]::StartNew()
for ($i = 0; $i -lt $Total; $i++) {
  $ps = [PowerShell]::Create().AddScript($pushScript).AddArgument($uri).AddArgument($Token).AddArgument($runId).AddArgument($i).AddArgument($BaseUrl)
  $ps.RunspacePool = $pool
  $jobs += [pscustomobject]@{ Pipe = $ps; Handle = $ps.BeginInvoke() }
}

$done = 0
$results = New-Object System.Collections.Generic.List[object]
while ($done -lt $jobs.Count) {
  for ($j = 0; $j -lt $jobs.Count; $j++) {
    $job = $jobs[$j]
    if ($null -eq $job) { continue }
    if ($job.Handle.IsCompleted) {
      $out = $job.Pipe.EndInvoke($job.Handle)
      $job.Pipe.Dispose()
      $jobs[$j] = $null
      $done++
      foreach ($r in @($out)) {
        $results.Add($r)
        if ($r.StatusCode -eq 200 -and $r.Accepted -ge 1) {
          [void]$script:latencies.Add([double]$r.Ms)
          $script:ok++
          $script:accepted += $r.Accepted
        } elseif ($r.StatusCode -eq 429) {
          $script:r429++
        } else {
          $script:fail++
        }
      }
      if (($done % 50) -eq 0 -or $done -eq $Total) {
        Write-Host ('progress {0}/{1}  ok={2} fail={3} 429={4}' -f $done, $Total, $script:ok, $script:fail, $script:r429) -ForegroundColor DarkGray
      }
    }
  }
  Start-Sleep -Milliseconds 20
}
$swAll.Stop()
$pool.Close()
$pool.Dispose()

$elapsed = [math]::Max(0.001, $swAll.Elapsed.TotalSeconds)
$rps = [math]::Round($Total / $elapsed, 1)

function Pct([System.Collections.Generic.List[double]]$arr, [double]$p) {
  if ($arr.Count -eq 0) { return 0 }
  $sorted = $arr | Sort-Object
  $idx = [math]::Min($sorted.Count - 1, [int][math]::Floor(($p / 100.0) * ($sorted.Count - 1)))
  return [math]::Round($sorted[$idx], 1)
}

Write-Host ''
Write-Host '=== Push Summary ===' -ForegroundColor Cyan
Write-Host ('elapsed_s={0:N2}  rps={1}  ok={2}  fail={3}  429={4}  accepted={5}' -f $elapsed, $rps, $script:ok, $script:fail, $script:r429, $script:accepted)
if ($script:latencies.Count -gt 0) {
  Write-Host ('latency_ms p50={0} p95={1} p99={2} max={3}' -f (Pct $script:latencies 50), (Pct $script:latencies 95), (Pct $script:latencies 99), ([math]::Round(($script:latencies | Measure-Object -Maximum).Maximum, 1)))
}

# Sample verify: alerts exist + optional notify check
$verifyOk = 0
$verifyMiss = 0
$notifyOk = 0
$notifySkip = 0
$notifyFail = 0

try {
  $login = Invoke-RestMethod -Uri ($BaseUrl + '/api/auth/login') -Method POST -ContentType 'application/json' -Body '{"username":"admin","password":"admin123"}'
  $auth = @{ Authorization = ('Bearer {0}' -f $login.token) }
  Start-Sleep -Seconds 2

  $sample = @($results | Where-Object { $_.StatusCode -eq 200 -and $_.Accepted -ge 1 } | Select-Object -First $VerifySample)
  foreach ($s in $sample) {
    $q = [uri]::EscapeDataString($s.Fp)
    $alPath = '/api/alerts?q=' + $q + '&limit=5&store=mysql'
    try {
      $al = Invoke-RestMethod -Uri ($BaseUrl + $alPath) -Headers $auth
      $items = @()
      if ($al.items) { $items = @($al.items) }
      elseif ($al -is [System.Array]) { $items = @($al) }
      $hit = $items | Where-Object { $_.fingerprint -like ('*{0}*' -f $s.Fp) } | Select-Object -First 1
      if (-not $hit -and $items.Count -gt 0) { $hit = $items[0] }
      if ($hit) {
        $verifyOk++
        if ($Notify) {
          try {
            $ns = Invoke-RestMethod -Uri ($BaseUrl + '/api/alerts/' + $hit.id + '/notifies') -Headers $auth
            $arr = @()
            if ($ns -is [System.Array]) { $arr = @($ns) } elseif ($ns.items) { $arr = @($ns.items) } else { $arr = @($ns) }
            $good = $arr | Where-Object { $_.success -eq $true } | Select-Object -First 1
            if ($good) { $notifyOk++ }
            elseif ($arr.Count -eq 0) { $notifySkip++ }
            else { $notifyFail++ }
          } catch {
            $notifyFail++
          }
        }
      } else {
        $verifyMiss++
      }
    } catch {
      $verifyMiss++
    }
  }
} catch {
  Write-Host ('verify login/query failed: {0}' -f $_.Exception.Message) -ForegroundColor Yellow
}

Write-Host ''
Write-Host '=== Verify Sample ===' -ForegroundColor Cyan
Write-Host ('sample={0}  alert_found={1}  alert_miss={2}' -f $VerifySample, $verifyOk, $verifyMiss)
if ($Notify) {
  Write-Host ('notify_success={0}  notify_none={1}  notify_fail={2}  (storm throttle/aggregate may skip)' -f $notifyOk, $notifySkip, $notifyFail)
}

Write-Host ''
Write-Host ('run_id={0}  fingerprints like load-{0}-NNNNN' -f $runId) -ForegroundColor DarkGray

if ($script:fail -gt 0 -and $script:ok -eq 0) {
  Write-Host 'LOAD FAILED' -ForegroundColor Red
  exit 1
}
Write-Host 'LOAD DONE' -ForegroundColor Green
exit 0
