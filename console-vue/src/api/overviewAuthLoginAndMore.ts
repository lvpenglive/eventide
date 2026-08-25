import { request } from './request'
import type {
  LoginResp,
  MeResp,
  ChangePasswordResp,
  OverviewResp,
  UiBetaToggleResp,
} from './types'

/**
 * 登录：POST /api/auth/login
 */
export function login(body: { username: string; password: string }): Promise<LoginResp> {
  return request<LoginResp>('/api/auth/login', {
    method: 'POST',
    body: JSON.stringify(body),
    skipAuth: true,
  })
}

/**
 * 当前用户：GET /api/auth/me
 */
export function me(): Promise<MeResp> {
  return request<MeResp>('/api/auth/me', {
    method: 'GET',
  })
}

/**
 * 修改密码：POST /api/auth/change-password
 */
export function changePassword(
  oldPassword: string,
  newPassword: string,
): Promise<ChangePasswordResp> {
  return request<ChangePasswordResp>('/api/auth/change-password', {
    method: 'POST',
    body: JSON.stringify({ old_password: oldPassword, new_password: newPassword }),
  })
}

/**
 * 总览数据：GET /api/overview
 */
export function overview(): Promise<OverviewResp> {
  return request<OverviewResp>('/api/overview', {
    method: 'GET',
  })
}

/**
 * 读取 Beta UI 开关：GET /api/settings/ui-betatoggle
 */
export function getUiBetaToggle(): Promise<UiBetaToggleResp> {
  return request<UiBetaToggleResp>('/api/settings/ui-betatoggle', {
    method: 'GET',
  })
}

/**
 * 更新 Beta UI 开关：PUT /api/settings/ui-betatoggle
 */
export function putUiBetaToggle(enabled: boolean): Promise<UiBetaToggleResp> {
  return request<UiBetaToggleResp>('/api/settings/ui-betatoggle', {
    method: 'PUT',
    body: JSON.stringify({ enabled }),
  })
}
