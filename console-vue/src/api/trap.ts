// B4 批次：Trap BFF API 模块
// 分两类端点：
//   1) /trap-api/api/*  —— Trap BFF 健康/统计/最近消息/模拟  (GET + POST)
//   2) /api/trap/*      —— 主后端 Trap 集群实例列表 (GET)
// 注意：trap-api 系列不需要除标准 JSON header 之外的自定义参数；
//       request() 自动附带 Authorization，保持鉴权链路一致。
import { request } from './request'
import type {
  B4TrapHealth,
  B4TrapStats,
  B4TrapRecentList,
  B4TrapClusterResp,
  B4TrapSimulateReq,
  B4TrapSimulateResp,
} from './types'

/**
 * 文件下载辅助：将 Blob 或 fetch Response 触发浏览器"另存为"
 * （若是 Response，则异步 .blob() 后再触发；函数仍返回 void 兼容调用方）
 * @param res  Blob 或者完整的 Response 对象
 * @param defaultName  Content-Disposition 缺失时使用的默认文件名
 */
export function downloadBlob(res: Blob | Response, defaultName: string): void {
  const doSave = (blob: Blob, filename: string) => {
    const url = URL.createObjectURL(blob)
    try {
      const a = document.createElement('a')
      a.href = url
      a.download = filename ?? 'download'
      a.rel = 'noopener'
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
    } finally {
      setTimeout(() => URL.revokeObjectURL(url), 10_000)
    }
  }

  if (res instanceof Response) {
    let filename = defaultName
    const disp =
      res.headers && typeof res.headers.get === 'function'
        ? res.headers.get('Content-Disposition')
        : null
    if (disp) {
      const m =
        /filename\*=UTF-8''([^;]+)/i.exec(disp) ||
        /filename="?([^";]+)"?/i.exec(disp)
      if (m?.[1]) {
        try {
          filename = decodeURIComponent(m[1])
        } catch {
          filename = m[1]
        }
      }
    }
    res
      .blob()
      .then(b => doSave(b, filename))
      .catch(() => {
        // 兜底：构造错误提示 blob
        doSave(
          new Blob([`Download failed (${res.status ?? 'unknown'})`], {
            type: 'text/plain',
          }),
          filename,
        )
      })
  } else {
    doSave(res, defaultName)
  }
}

/**
 * Trap BFF 健康检查：GET /trap-api/api/health
 */
export function getTrapHealth(): Promise<B4TrapHealth> {
  return request<B4TrapHealth>('/trap-api/api/health', {
    method: 'GET',
  })
}

/**
 * Trap BFF 统计：GET /trap-api/api/stats
 */
export function getTrapStats(): Promise<B4TrapStats> {
  return request<B4TrapStats>('/trap-api/api/stats', {
    method: 'GET',
  })
}

/**
 * Trap BFF 最近 Trap 消息列表：GET /trap-api/api/recent
 */
export function getTrapRecent(): Promise<B4TrapRecentList> {
  return request<B4TrapRecentList>('/trap-api/api/recent', {
    method: 'GET',
  })
}

/**
 * 主后端列出 Trap 集群实例：GET /api/trap/instances
 */
export function listTrapInstances(): Promise<B4TrapClusterResp> {
  return request<B4TrapClusterResp>('/api/trap/instances', {
    method: 'GET',
  })
}

/**
 * Trap BFF 模拟发送一条 Trap：POST /trap-api/api/simulate
 */
export function simulateTrap(body: B4TrapSimulateReq): Promise<B4TrapSimulateResp> {
  const payload: B4TrapSimulateReq = {
    ip: body?.ip ?? '',
    trap_oid: body?.trap_oid ?? '',
  }
  if (body?.alertname !== undefined) payload.alertname = body.alertname
  if (body?.severity !== undefined && body.severity !== '') payload.severity = body.severity
  if (body?.dry_run !== undefined) payload.dry_run = body.dry_run
  return request<B4TrapSimulateResp>('/trap-api/api/simulate', {
    method: 'POST',
    body: JSON.stringify(payload),
  })
}
