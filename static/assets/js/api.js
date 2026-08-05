/* Eventide console — API client */
export const TOKEN_KEY = "eventide_token";
export const USER_KEY = "eventide_user";

let onUnauthorized = () => {};

/** Called on HTTP 401 (except login). Wired from app.js to logout(false). */
export function setUnauthorizedHandler(fn) {
  onUnauthorized = typeof fn === "function" ? fn : () => {};
}

export function token() {
  return localStorage.getItem(TOKEN_KEY);
}

export async function api(path, opts = {}) {
  const headers = Object.assign({}, opts.headers || {});
  if (!(opts.body instanceof FormData) && opts.body && !headers["Content-Type"]) {
    headers["Content-Type"] = "application/json";
  }
  const t = token();
  if (t) headers.Authorization = `Bearer ${t}`;
  const res = await fetch(path, { ...opts, headers });
  if (res.status === 401 && !path.includes("/api/auth/login")) {
    onUnauthorized();
    throw new Error("登录已失效");
  }
  if (res.status === 204) return null;
  const text = await res.text();
  let data = null;
  try {
    data = text ? JSON.parse(text) : null;
  } catch {
    data = { error: text };
  }
  if (!res.ok) {
    const err = new Error((data && data.error) || text || res.statusText);
    if (data && data.code) err.code = data.code;
    if (res.status === 402 || data?.code === "license_readonly") {
      err.code = "license_readonly";
      err.message =
        (data && data.error) ||
        "许可证无效或已过期，当前为只读宽限。请到「系统设置」导入许可证。";
    }
    throw err;
  }
  return data;
}
