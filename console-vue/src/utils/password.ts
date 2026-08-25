export interface PasswordValidationResult {
  ok: boolean
  message: string
}

/**
 * 密码复杂度校验（与 ui.js validatePasswordComplexity 同语义）：
 * - 至少 8 位
 * - 同时包含大写字母、小写字母、数字
 * - 同时包含特殊字符（非大小写字母、非数字）
 */
export function validatePasswordComplexity(password: string): PasswordValidationResult {
  if (!password) {
    return { ok: false, message: '请输入密码' }
  }
  if ([...password].length < 8) {
    return { ok: false, message: '密码至少 8 位' }
  }
  if (!/[a-z]/.test(password)) {
    return { ok: false, message: '密码须包含小写字母' }
  }
  if (!/[A-Z]/.test(password)) {
    return { ok: false, message: '密码须包含大写字母' }
  }
  if (!/[0-9]/.test(password)) {
    return { ok: false, message: '密码须包含数字' }
  }
  if (!/[^A-Za-z0-9]/.test(password)) {
    return { ok: false, message: '密码须包含特殊字符（如 !@#$%）' }
  }
  return { ok: true, message: '' }
}

export default validatePasswordComplexity
