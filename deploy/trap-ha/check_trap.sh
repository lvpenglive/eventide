#!/usr/bin/env bash
# Keepalived MISC_CHECK: exit 0 = healthy, non-zero = fail (lower priority / failover).
set -euo pipefail
HTTP_URL="${TRAP_HEALTH_URL:-http://127.0.0.1:8081/api/health}"
curl -fsS --max-time 2 "$HTTP_URL" | grep -q '"status"[[:space:]]*:[[:space:]]*"ok"'
