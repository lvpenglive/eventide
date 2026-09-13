# Eventide overall smoke (API level)
# Covers: health, login, core list APIs, settings, ingress -> MeridianOps notify chain
#
# Usage:
#   powershell -File scripts/smoke-eventide.ps1
# Env:
#   EVENTIDE_URL / EVENTIDE_USER / EVENTIDE_PASS
#   MERIDIAN_CHANNEL (default MeridianOps)
#   SKIP_MERIDIAN=1 to skip notify e2e

$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$base = if ($env:EVENTIDE_URL) { $env:EVENTIDE_URL.TrimEnd('/') } else { 'http://127.0.0.1:8080' }
$user = if ($env:EVENTIDE_USER) { $env:EVENTIDE_USER } else { 'admin' }
$pass = if ($env:EVENTIDE_PASS) { $env:EVENTIDE_PASS } else { 'admin123' }
$meridianName = if ($env:MERIDIAN_CHANNEL) { $env:MERIDIAN_CHANNEL } else { 'MeridianOps' }
$skipMeridian = $env:SKIP_MERIDIAN -eq '1'

$script:passCount = 0
$script:failCount = 0
$script:skipCount = 0
$script:rows = New-Object System.Collections.Generic.List[object]

function Add-Result([string]$name, [string]$status, [string]$detail = '') {
  $script:rows.Add([pscustomobject]@{ Name = $name; Status = $status; Detail = $detail })
  switch ($status) {
    'PASS' { $script:passCount++ }
    'FAIL' { $script:failCount++ }
    default { $script:skipCount++ }
  }
  $color = switch ($status) { 'PASS' { 'Green' } 'FAIL' { 'Red' } default { 'Yellow' } }
  Write-Host ('[{0}] {1}' -f $status, $name) -ForegroundColor $color
  if ($detail) { Write-Host ('       {0}' -f $detail) -ForegroundColor DarkGray }
}

function Get-JsonText($obj) {
  if ($obj -is [string]) { return $obj }
  return ($obj | ConvertTo-Json -Depth 12 -Compress)
}

function ApiRaw([string]$method, [string]$path, $headers = $null, $bodyObj = $null, [int]$timeoutSec = 20) {
  $uri = $base + $path
  $params = @{
    Uri             = $uri
    Method          = $method
    UseBasicParsing = $true
    TimeoutSec      = $timeoutSec
  }
  if ($headers) { $params.Headers = $headers }
  if ($null -ne $bodyObj) {
    $params.ContentType = 'application/json'
    $params.Body = Get-JsonText $bodyObj
  }
  return Invoke-WebRequest @params
}

function ApiJson([string]$method, [string]$path, $headers = $null, $bodyObj = $null) {
  $resp = ApiRaw $method $path $headers $bodyObj
  if ([string]::IsNullOrWhiteSpace($resp.Content)) { return $null }
  return $resp.Content | ConvertFrom-Json
}

function As-List($x) {
  if ($null -eq $x) { return @() }
  # Check Array BEFORE .items — PS member enumeration makes $arr.items a non-empty
  # null array which is truthy and would wipe object fields.
  if ($x -is [System.Array]) { return @($x) }
  $propNames = @($x.PSObject.Properties | ForEach-Object { $_.Name })
  if ($propNames -contains 'items') { return @($x.items) }
  return @($x)
}

Write-Host ''
Write-Host '=== Eventide Smoke ===' -ForegroundColor Cyan
Write-Host ('base = {0}' -f $base)
Write-Host ''

# ---------- 1. Health ----------
try {
  $h = ApiJson 'GET' '/api/health'
  if ($h.status -eq 'ok' -or $h.service -eq 'eventide') {
    Add-Result 'health' 'PASS' ($h | ConvertTo-Json -Compress)
  } else {
    Add-Result 'health' 'FAIL' ($h | ConvertTo-Json -Compress)
  }
} catch {
  Add-Result 'health' 'FAIL' $_.Exception.Message
  Write-Host ''
  Write-Host 'Backend unreachable; abort.' -ForegroundColor Red
  exit 2
}

# ---------- 2. Login ----------
$auth = $null
$token = $null
try {
  $loginBody = '{"username":"' + $user + '","password":"' + $pass + '"}'
  $login = ApiJson 'POST' '/api/auth/login' $null $loginBody
  $token = $login.token
  if (-not $token) { throw 'login response missing token' }
  $auth = @{ Authorization = ('Bearer {0}' -f $token) }
  Add-Result 'auth/login' 'PASS' ('user={0} perms={1}' -f $login.username, (($login.permissions) -join ','))
} catch {
  Add-Result 'auth/login' 'FAIL' $_.Exception.Message
  exit 2
}

# ---------- 3. Core read APIs ----------
$checks = @(
  @{ Name = 'overview'; Path = '/api/overview' },
  @{ Name = 'datasources'; Path = '/api/datasources' },
  @{ Name = 'rules'; Path = '/api/rules' },
  @{ Name = 'channels'; Path = '/api/channels' },
  @{ Name = 'ingress'; Path = '/api/ingress' },
  @{ Name = 'alerts'; Path = '/api/alerts?limit=5&store=mysql' },
  @{ Name = 'silences'; Path = '/api/silences' },
  @{ Name = 'maintenance'; Path = '/api/maintenance-windows' },
  @{ Name = 'notifies'; Path = '/api/notifies?limit=5' },
  @{ Name = 'audit-logs'; Path = '/api/audit-logs?limit=5' },
  @{ Name = 'settings/storm'; Path = '/api/settings/storm' },
  @{ Name = 'settings/alert-history'; Path = '/api/settings/alert-history' },
  @{ Name = 'settings/trap-token'; Path = '/api/settings/trap-token' },
  @{ Name = 'license'; Path = '/api/license' }
)

$channels = @()
$ingressRoutes = @()

foreach ($c in $checks) {
  try {
    $data = ApiJson 'GET' $c.Path $auth
    $detail = ''
    if ($c.Name -eq 'channels') {
      $channels = As-List $data
      $detail = ('count={0}' -f $channels.Count)
    } elseif ($c.Name -eq 'ingress') {
      $ingressRoutes = As-List $data
      $detail = ('count={0}' -f $ingressRoutes.Count)
    } elseif ($c.Name -eq 'alerts') {
      $detail = ('total={0}' -f $data.total)
    } elseif ($c.Name -eq 'overview') {
      $detail = ('firing={0}' -f $data.alerts_firing)
    } elseif ($c.Name -eq 'settings/storm') {
      $detail = ('source={0} throttle={1}' -f $data.source, $data.throttle_enabled)
    } else {
      $n = (As-List $data).Count
      if ($n -gt 0) { $detail = ('count={0}' -f $n) }
    }
    Add-Result ('GET {0}' -f $c.Name) 'PASS' $detail
  } catch {
    Add-Result ('GET {0}' -f $c.Name) 'FAIL' $_.Exception.Message
  }
}

# ---------- 4. v2 static ----------
try {
  $v2 = ApiRaw 'GET' '/v2/' $null $null 10
  if ($v2.StatusCode -ge 200 -and $v2.StatusCode -lt 400) {
    Add-Result 'GET /v2/' 'PASS' ('status={0}' -f $v2.StatusCode)
  } else {
    Add-Result 'GET /v2/' 'FAIL' ('status={0}' -f $v2.StatusCode)
  }
} catch {
  Add-Result 'GET /v2/' 'FAIL' $_.Exception.Message
}

# ---------- 5. MeridianOps chain ----------
if ($skipMeridian) {
  Add-Result 'meridian/channel-test' 'SKIP' 'SKIP_MERIDIAN=1'
  Add-Result 'meridian/ingress-e2e' 'SKIP' 'SKIP_MERIDIAN=1'
} else {
  $ch = @($channels) | Where-Object {
    $_.name -and ($_.name.ToString().Trim() -ieq $meridianName.Trim())
  } | Select-Object -First 1
  if (-not $ch) {
    $names = (@($channels) | ForEach-Object { $_.name }) -join ', '
    Add-Result 'meridian/channel-test' 'FAIL' ('channel not found: {0}; have=[{1}]' -f $meridianName, $names)
    Add-Result 'meridian/ingress-e2e' 'SKIP' 'no channel'
  } elseif (-not $ch.enabled) {
    Add-Result 'meridian/channel-test' 'FAIL' 'channel disabled'
    Add-Result 'meridian/ingress-e2e' 'SKIP' 'channel disabled'
  } else {
    try {
      $ct = $null
      $lastErr = ''
      foreach ($attempt in 1..3) {
        try {
          $ct = ApiJson 'POST' ('/api/channels/{0}/test' -f $ch.id) $auth
          if ($ct.ok -eq $true) { break }
          $lastErr = ('err={0} http={1}' -f $ct.error, $ct.http_status)
        } catch {
          $lastErr = $_.Exception.Message
          $ct = $null
        }
        if ($attempt -lt 3) { Start-Sleep -Seconds 2 }
      }
      if ($ct -and $ct.ok -eq $true) {
        Add-Result 'meridian/channel-test' 'PASS' ('http={0} url={1}' -f $ct.http_status, $ct.request_url)
      } else {
        Add-Result 'meridian/channel-test' 'FAIL' $lastErr
      }
    } catch {
      Add-Result 'meridian/channel-test' 'FAIL' $_.Exception.Message
    }

    $route = $ingressRoutes | Where-Object {
      $_.enabled -and ($_.channel_ids -contains $ch.id)
    } | Select-Object -First 1

    if (-not $route) {
      Add-Result 'meridian/ingress-e2e' 'FAIL' ('no enabled ingress bound to {0}' -f $meridianName)
    } else {
      $fp = 'smoke-{0}' -f (Get-Date -Format 'yyyyMMdd-HHmmss')
      $tokenHdr = if ($route.token) { $route.token } else { '' }
      try {
        $ingHeaders = @{
          'X-Eventide-Token' = $tokenHdr
          'Content-Type'     = 'application/json; charset=utf-8'
        }
        if ($route.kind -eq 'alertmanager') {
          $path = '/api/ingress/{0}/alertmanager' -f $route.id
          $bodyObj = @{
            alerts = @(
              @{
                status       = 'firing'
                labels       = @{ alertname = ('Smoke-{0}' -f $fp); severity = 'warning'; instance = 'smoke' }
                annotations  = @{ summary = ('Eventide smoke test {0}' -f $fp) }
                fingerprint  = $fp
              }
            )
          }
        } else {
          $path = '/api/ingress/{0}/generic' -f $route.id
          $bodyObj = @{
            status      = 'firing'
            alertname   = ('Smoke-{0}' -f $fp)
            severity    = 'warning'
            ip          = '10.0.0.88'
            summary     = ('Eventide smoke test {0}' -f $fp)
            fingerprint = $fp
          }
        }
        $ingBody = Get-JsonText $bodyObj
        $ingResp = Invoke-WebRequest -Uri ($base + $path) -Method POST -Headers $ingHeaders -Body $ingBody -UseBasicParsing -TimeoutSec 20
        $ingJson = $ingResp.Content | ConvertFrom-Json
        if ($ingResp.StatusCode -ne 200 -or [int]$ingJson.accepted -lt 1) {
          Add-Result 'meridian/ingress-e2e' 'FAIL' ('ingress accepted={0} body={1}' -f $ingJson.accepted, $ingResp.Content)
        } else {
          Start-Sleep -Seconds 2
          $q = [uri]::EscapeDataString($fp)
          $alertsPath = '/api/alerts?q=' + $q + '&limit=5&store=mysql'
          $al = ApiJson 'GET' $alertsPath $auth
          $items = As-List $al
          $alert = $items | Where-Object { $_.fingerprint -like ('*{0}*' -f $fp) } | Select-Object -First 1
          if (-not $alert -and $items.Count -gt 0) { $alert = $items[0] }
          if (-not $alert) {
            Add-Result 'meridian/ingress-e2e' 'FAIL' ('alert not found for fp={0}' -f $fp)
          } else {
            $notifies = As-List (ApiJson 'GET' ('/api/alerts/{0}/notifies' -f $alert.id) $auth)
            $okN = $notifies | Where-Object { $_.success -eq $true -and $_.channel_id -eq $ch.id } | Select-Object -First 1
            if ($okN) {
              Add-Result 'meridian/ingress-e2e' 'PASS' ('route={0} alert={1} transition={2} fp={3}' -f $route.name, $alert.id, $okN.transition, $fp)
            } else {
              $last = $notifies | Select-Object -First 1
              $err = if ($last) { $last.error } else { 'no notify log' }
              Add-Result 'meridian/ingress-e2e' 'FAIL' ('alert={0} notify_err={1}' -f $alert.id, $err)
            }
          }
        }
      } catch {
        Add-Result 'meridian/ingress-e2e' 'FAIL' $_.Exception.Message
      }
    }
  }
}

# ---------- Summary ----------
Write-Host ''
Write-Host '=== Summary ===' -ForegroundColor Cyan
Write-Host ('PASS={0}  FAIL={1}  SKIP={2}' -f $script:passCount, $script:failCount, $script:skipCount)
Write-Host ''
$script:rows | Format-Table -AutoSize | Out-String | Write-Host

if ($script:failCount -gt 0) {
  Write-Host 'SMOKE FAILED' -ForegroundColor Red
  exit 1
}
Write-Host 'SMOKE PASSED' -ForegroundColor Green
exit 0
