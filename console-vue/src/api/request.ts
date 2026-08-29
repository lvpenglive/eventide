export interface RequestOptions extends RequestInit {
  query?: Record<string, string | number | boolean | undefined | null>
  skipAuth?: boolean
}

export class RequestError extends Error {
  status: number
  data?: unknown

  constructor(message: string, status: number, data?: unknown) {
    super(message)
    this.name = 'RequestError'
    this.status = status
    this.data = data
  }
}

function buildUrl(url: string, query?: RequestOptions['query']): string {
  if (!query) return url
  const params = new URLSearchParams()
  for (const [k, v] of Object.entries(query)) {
    if (v === undefined || v === null) continue
    params.append(k, String(v))
  }
  const qs = params.toString()
  return qs ? `${url}${url.includes('?') ? '&' : '?'}${qs}` : url
}

export async function request<T = unknown>(
  url: string,
  options: RequestOptions = {}
): Promise<T> {
  const { query, skipAuth, headers, body, method, ...rest } = options

  const finalHeaders: Record<string, string> = {
    Accept: 'application/json',
    ...(headers as Record<string, string> | undefined),
  }

  if (body && !(body instanceof FormData)) {
    finalHeaders['Content-Type'] = 'application/json'
  }

  if (!skipAuth) {
    const token = localStorage.getItem('eventide_token')
    if (token) {
      finalHeaders['Authorization'] = `Bearer ${token}`
    }
  }

  const finalUrl = buildUrl(url, query)
  let response: Response
  try {
    response = await fetch(finalUrl, {
      method: method || (body ? 'POST' : 'GET'),
      headers: finalHeaders,
      body,
      ...rest,
    })
  } catch (e: unknown) {
    const message =
      e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : '网络请求失败'
    throw new RequestError(message, 0)
  }

  if (response.status === 401) {
    localStorage.removeItem('eventide_token')
    localStorage.removeItem('eventide_user')
    window.dispatchEvent(new CustomEvent('auth:unauthorized'))
    throw new RequestError('未登录或登录已过期', 401)
  }

  let data: unknown
  const text = await response.text()
  try {
    data = text ? JSON.parse(text) : null
  } catch {
    data = text
  }

  if (!response.ok) {
    const message =
      (data && typeof data === 'object' && 'message' in data && typeof (data as { message: unknown }).message === 'string')
        ? (data as { message: string }).message
        : (data && typeof data === 'object' && 'error' in data && typeof (data as { error: unknown }).error === 'string')
          ? (data as { error: string }).error
          : `请求失败 (${response.status})`
    throw new RequestError(message, response.status, data)
  }

  return data as T
}

export default request
