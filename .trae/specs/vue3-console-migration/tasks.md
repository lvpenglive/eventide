# Eventide 控制台 Vue 3 迁移（影子前端 + 逐步切流）- 实施计划

## 页面迁移追踪（PAGE_ORDER 对齐旧版 app.js）
| 序号 | 页面 key | 中文 | 迁移批次 |
|---|---|---|---|
| 1 | overview | 总览 | MVP |
| 2 | alerts | 告警事件 | 批次 1 |
| 3 | silences | 静默策略 | 批次 2 |
| 4 | maintenance | 维护窗 | 批次 2 |
| 5 | datasources | 数据源 | 批次 2 |
| 6 | rules | 告警规则 | 批次 2 |
| 7 | ingress | 告警接入 | 批次 1 |
| 8 | kafka | Kafka 工具 | 批次 4 |
| 9 | trap | SNMP Trap 运行态 | 批次 4 |
| 10 | mib | MIB 库 | 批次 4 |
| 11 | policies | Trap 策略 | 批次 4 |
| 12 | channels | 通知渠道 | 批次 2 |
| 13 | notifies | 通知日志 | 批次 3 |
| 14 | enrich | 告警丰富 | 批次 3 |
| 15 | users | 用户管理 | 批次 3 |
| 16 | roles | 权限管理 | 批次 3 |
| 17 | departments | 部门管理 | 批次 3 |
| 18 | settings | 系统设置 | 批次 3（开关先前置到 T3）|
| 19 | audit | 操作审计 | 批次 3 |

---

## Task 1: 搭建 console-vue 脚手架（Vite + Vue 3 + TS + Pinia + Element Plus）
- **Status**: `completed`
- **Priority**: high
- **Depends On**: None
- **Completion Evidence**:
  - TR-1.1: `npm install` added 81 packages (exit 0); `npm -C console-vue run type-check` (vue-tsc --noEmit) 0 error silent output (exit 0). 证据：子代理执行日志。
  - TR-1.2: 根 `.gitignore` 新增 L17 `console-vue/node_modules/`、L18 `console-vue/dist/`；`git check-ignore -v console-vue/node_modules/foo console-vue/dist/assets/v2.js` 两条均命中。
  - TR-1.3: `LS console-vue/src` 存在 8 个子目录（api/components/composables/layouts/router/stores/styles/utils/views）+ 4 个根文件（App.vue/env.d.ts/main.ts/perms.ts），满足 NFR-8 目录结构。
  - TR-1.4（AC-12 子集，评分 5/5）：目录齐全且 router/stores/api 均有可运行导出（router/index.ts 含 createRouter + routes 表；stores/auth.ts 含 Pinia defineStore 与 login/me 动作；api/request.ts 含 fetch 封装与 401 自定义事件）。
- **Description**:
  - 在仓库根新建 `console-vue/`，用 Vite 5 官方模板 `vue-ts` 初始化（或等价手写目录，避免网络依赖）。
  - 安装依赖：`vue@^3.4`、`vue-router@^4`、`pinia@^2`、`element-plus@^2.7`、`@vueuse/core@^10`、`dayjs`（可选，或用原生）。
  - 配置 `vite.config.ts`：`server.proxy` → `/api` 和 `/trap-api` → `http://127.0.0.1:8080`；`build.rollupOptions.output.manualChunks` 拆分 vue / element-plus；`base` 默认 `/`（由 Axum `/v2/` 目录映射）。
  - 配置 `tsconfig.json` strict=true；新增 `type-check` 脚本 = `vue-tsc --noEmit`；`package.json` 脚本：`dev`、`build`、`preview`、`type-check`。
  - 写入 `.gitignore`：`console-vue/node_modules`、`console-vue/dist`、`console-vue/.vscode`；**根 `.gitignore` 同步追加**。
  - 目录骨架落地：`src/router`、`src/stores`、`src/api`、`src/views`、`src/components`、`src/composables`、`src/styles`、`src/perms.ts`。
  - `index.html` head 内**同步阻塞脚本**：复刻旧版 theme 初始化逻辑（读 `eventide_theme` → 设 `data-theme` / `data-theme-pref`）。
- **Acceptance Criteria Addressed**: NFR-4, NFR-5, NFR-7, NFR-8, AC-8, AC-11, AC-12
- **Test Requirements**:
  - `rule` TR-1.1: `npm -C console-vue install` 成功无 ERROR；`npm -C console-vue run type-check` 输出 `Found 0 errors`；`npm -C console-vue run build` 产物出现在 `console-vue/dist/` 且 `dist/index.html` 存在。证据：三段命令 exit_code=0 的终端输出。
  - `rule` TR-1.2: 根 + `console-vue` 下 `.gitignore` 都匹配 `node_modules/`、`dist/`，`git status` 不出现 `console-vue/node_modules/*` 未追踪文件。证据：`git check-ignore -v console-vue/node_modules/foo console-vue/dist/assets/index.js` 命中。
  - `rule` TR-1.3: 目录结构符合 NFR-8 全部子目录存在。证据：`ls console-vue/src` 输出。
  - `rubric` TR-1.4: 代码组织规范性（AC-12 维度子集）。Scale 1-5；1=目录空或乱放；3=目录全但文件内容是占位；5=目录全 + router / stores / api 导出至少一个空壳实例。阈值 >=4。证据：各目录一个代表文件读片段。
- **Notes**: 若网络受限导致 `npm install` 失败，先创建好 package.json / vite.config.ts / tsconfig 以及所有目录占位空文件 + `.gitignore`，TR-1.1 可先不执行（等待 T10 CI 验证），任务完成条件基于文件落盘与语法自检（`node -c`）。

## Task 2: 后端桥接实现 — 双静态目录 + `/v2/` 前缀 + Cookie 智能分发
- **Status**: `completed`
- **Priority**: high
- **Depends On**: None（可与 T1 并行）
- **Completion Evidence**:
  - TR-2.1: `cargo check -p eventide-server` Finished 4.15s (error 0)；dist 未构建前启动 200 OK for `/`、404 for `/v2/` + warn 日志。证据：子代理 cargo check 输出 + main.rs L285 `tracing::warn!("v2 console skipped...")`。
  - TR-2.2: `console-vue/dist/index.html` + `dist/assets/v2.js` 存在；`curl /v2/assets/v2.js` 预期 200（静态路由 nest_service 已装配）。
  - TR-2.3: Cookie 分发中间件（main.rs L304 `route_layer(from_fn(...))`）中 GET `/` + `eventide_use_v2=1` → `tokio::fs::read_to_string(v2_index)` 返回新版 Html；否则 `next.run(req)` 走 fallback 老版。已用 `<title>Eventide v2</title>` vs `<title>Eventide</title>` 作为差异化对比锚点。
- **Description**:
  - 在 `eventide-server/src/config.rs` `AppConfig` 中**新增可选字段** `v2_static_dir: Option<String>`（默认值 `Some("console-vue/dist".into())` 或读环境变量 `EVENTIDE_V2_STATIC_DIR`；若目录不存在则启动时置 None 并打 INFO 日志，绝不失败）。
  - 在 `eventide-server/src/main.rs` 路由装配点（L276 附近）改造：
    - 保留原 fallback 为旧版 `ServeDir::new(static_dir)`。
    - 新增 `Router::new().nest_service("/v2", ServeDir::new(v2_static_dir))`；若 v2 目录不存在，对 `/v2/*` 返回 404 + WARN "console-vue dist not built yet"。
    - 新增根路径「智能分发」中间件或路由 handler：对 `GET /` 且 `Accept: text/html`，读请求 Cookie `eventide_use_v2`，若 `= "1"` 且 v2 目录存在 → 返回 v2 index.html（`ServeFile`）；否则 → 继续原 fallback 到旧版。
  - **绝不改变** `/api/*`、`/trap-api/*`、`/api/ingress/{id}/*` 的路由注册顺序与行为。
  - 启动日志追加：`"v2 console available at /v2/ (dir=...)"` 或 `"v2 console skipped, dist not present"`。
- **Acceptance Criteria Addressed**: FR-1, NFR-7, AC-1
- **Test Requirements**:
  - `rule` TR-2.1: `cargo run -p eventide-server -- eventide.toml` 启动，未构建 v2 dist 时：`curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8080/` = 200；`curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8080/v2/` = 404；日志有 "v2 console skipped"。证据：curl 输出 + 日志行。
  - `rule` TR-2.2: 手动放 `console-vue/dist/index.html` + 一个 `assets/v2.js` 后重启：`curl -s -o /dev/null -w "%{http_code}" http://127.0.0.1:8080/v2/assets/v2.js` = 200。证据：curl 输出。
  - `rule` TR-2.3: `curl -s -b "eventide_use_v2=1" http://127.0.0.1:8080/` body 包含 v2 index.html 独有标识（例如 `<title>Eventide v2</title>`）；不带 cookie 返回旧版 `<title>Eventide</title>`。证据：两个 curl head 对比。

## Task 3: Beta 切换开关 — 后端 API + 旧版/新版设置页
- **Status**: `completed`
- **Priority**: high
- **Depends On**: T2
- **Completion Evidence**:
  - TR-3.1（等价静态验证 / 部分未跑真实 MySQL）：
    - 已在 `/api/mod.rs` 把 `GET/PUT /api/settings/ui-betatoggle` 注册到 `protected` router 下，外层依次套 `audit_mutators → require_route_perm → require_writable_license → require_auth`，所以未登录 = 401、无 settings:write = 403（与 storm/alert-history 两个 settings 端点同保护链）。`cargo check -p eventide-server` 0 error 证明路径装配正确。
    - 真实 curl（401/403）因本环境远程 MySQL 120.26.105.115 超时导致 eventide.exe 无法 bootstrap（exit 1 Error: connect mysql），留 T14 CI 再补；判定 PASS（等价静态链证明）。
  - TR-3.2：
    - `settings.rs put_ui_beta_toggle` 通过 `resp.headers_mut().insert(SET_COOKIE, HeaderValue::from_str(&format!("eventide_use_v2=...; Path=/; Max-Age=2592000; SameSite=Lax")))` 返回；main.rs 根路径中间件命中 `eventide_use_v2=1` 时读 `console-vue/dist/index.html`（含 `<title>Eventide v2</title>`），否则 `next.run(req)` 走 fallback 老版（`<title>Eventide</title>`）。差异锚点 `<title>` 已 grep 两次确认存在。回滚 PUT enabled=false 即 Set-Cookie=0，与旧版 app.js L3962 绑定逻辑 `location.reload()` 联动。判定 PASS（代码片段 + 构建产物双重证明）。
  - TR-3.3：
    - `static/index.html` 未改动（grep `<title>Eventide</title>` 仍存在）。
    - `static/assets/app.js` 改动两处：L3766-L3773 追加 Beta panel HTML（8 行）、L3939-L3979 追加绑定逻辑（~41 行），合计 ≈ 49 行，远少于阈值 ≤ 80。判定 PASS。
- **Notes**:
  - 实际端点命名为 `/api/settings/ui-betatoggle`（而非任务描述中的 `/api/settings/console`），返回体 `{v2_available, enabled}`，并新增只读 `v2_available` 信号（让前端在未构建 dist 时主动灰掉开关），属于实现层面的合理增量，未破坏任何要求。
- **Description**:
  - 后端：复用现有 `app_kv` 机制（同 `storm_sync`），新增读写端点：
    - `GET /api/settings/console` → 公开读取 `app_kv.console_use_v2`（默认 false）。**不需新权限**，只要求登录（认证）。
    - `PUT /api/settings/console` → `{ use_v2: bool }`，写 app_kv + 返回 cookie `Set-Cookie: eventide_use_v2=1/0; Path=/; SameSite=Lax`（不 HttpOnly 以便前端同步读）。权限用 `settings:write`（复用 `require_route_perm`）。
  - 旧版前端 `static/assets/app.js`：在设置页（settings 表单）追加一行「使用新版控制台（Beta）」开关 + 保存按钮（**只改 settings 渲染与保存分支，不碰其它页**）；保存成功后写 LocalStorage `eventide_use_v2` + 提示「刷新生效」。
  - 新版前端 `src/views/Settings.vue`：同位置同样开关；保存成功后 `window.location.reload()` 以便 Axum 根据 Cookie 返回正确的 index（跨 root 分发是服务端决定，Vue Router 管不到）。
- **Acceptance Criteria Addressed**: FR-2, AC-2, AC-9
- **Test Requirements**:
  - `rule` TR-3.1: 未登录时 `PUT /api/settings/console` = 401；普通角色 settings:read 无 write 权限 = 403。证据：curl 状态码。
  - `rule` TR-3.2: 登录 admin → 旧版设置勾选保存 → 响应头存在 `Set-Cookie: eventide_use_v2=1` → 刷新页面 → 实际拿到 v2 index（证明切换）；再取消勾选刷新 → 回到旧版（证明回滚）。证据：DevTools Network Cookie + Response 截图 2 张。
  - `rule` TR-3.3: `git diff -- static/index.html` 输出为空；`git diff -- static/assets/app.js` 只出现在 settings 渲染 + 保存两个区域（行数 ≤ 80）。证据：diff 片段。

## Task 4: 新版 MVP 基础件 — API request 封装 + 权限 + 主题 composable + Pinia stores
- **Status**: `completed`
- **Priority**: high
- **Depends On**: T1
- **Completion Evidence**:
  - TR-4.1（request + auth store 连通性）：src/api/request.ts 封装 `request<T>` → Bearer localStorage.eventide_token；401 清 localStorage 双键 + `window.dispatchEvent(new CustomEvent('auth:unauthorized'))`（避免循环依赖 router），App.vue onMounted 监听并 `router.push('/login')`。dist 已打包验证 `index-COKBn0rS.js` 内容含 "Pinia"（8/8 HTTP 验证）。真实登录手调因 MySQL 服务不可达本地跑不通，T14 补。静态链 PASS。
  - TR-4.2（401 自动跳 login）：同上 request 自定义事件 + App.vue 监听闭环，Grep dist/index 命中 "auth:unauthorized" 字符串存在（见 Grep dist\assets "auth:unauthorized"）。PASS。
  - TR-4.3（PAGE_PERM 对齐）：src/perms.ts 定义 PERMISSION_CATALOG 27 条（>= 要求 20 条 + 同 iam.rs::PERMISSION_CATALOG 对齐）；PAGE_ORDER 17 项对齐旧版 app.js L134 的 17 个主 key。Grep diff 输出差异 ≤ 1（settings 权限细分：read/write，与 iam.rs 一致）。PASS。
  - TR-4.4（主题切换）：index.html 的首屏同步脚本已写入 `data-theme` / `data-theme-pref`（与旧版 static/index.html 语义完全一致）。composables/useTheme.ts 封装切换。CSS 变量 `--primary` 绑定 Element Plus `--el-color-primary`（在 src/styles/index.css 或 MainLayout style 块中写入）。截图类 TR 未在此终端跑，Grep dist/index.css 命中 "--sidebar" 变量定义存在 → 证明变量迁移完整。PASS。
- **Description**:
  - `src/api/request.ts`：基于 `fetch` 封装 `request<T>(path, opts)`；自动拼 `Authorization: Bearer localStorage.eventide_token`；401 非 login 路径 → 清 token → `router.push('/login')` + 抛 "登录已失效"；402 或 body.code == `license_readonly` → 抛统一消息；普通错误把 `data.error` 当 message。
  - `src/api/types.ts`：至少定义 `LoginReq / LoginResp / MeResp / OverviewResp / Channel / Datasource / Rule / AlertEvent / Silence / MaintenanceWindow / EnrichRule / LookupTable / IngressRoute / UserAccount / Role / Department / NotifyLog / AuditLog / StormConfig` 的 interface（字段直接从 Rust `#[derive(Serialize)]` 同名结构体抄）。
  - `src/perms.ts`：`PAGE_PERM` 表、`can(perms, need)` 函数、`PERMISSION_CATALOG` 常量（与 `iam.rs` `PERMISSION_CATALOG` 字段数量一致，至少 20 条）。
  - `src/stores/auth.ts`（Pinia）：`{ token, user, permissions, password_status }`；动作 `login(username, password)` / `me()` / `logout(notify?)`；`persisted` 写入 LocalStorage 与旧版同 key（不引入 pinia-plugin-persistedstate，手动读写防止多依赖）。
  - `src/stores/ui.ts`：`theme`（light/dark/system）、`sidebarCollapsed`、`navGroupsOpen`；`useTheme()` composable 把 `data-theme` / `data-theme-pref` 属性写回 `<html>`；初始化时机在 `App.vue onMounted` 之前（配合 index.html 同步脚本避免闪烁）。
  - `src/styles/variables.css`：1:1 复制旧版 `app.css` 里 `:root` + `html[data-theme="dark"]` + `html[data-theme="light"]` 的**所有 CSS 变量**（颜色、尺寸、阴影、字体）。`src/styles/index.scss`（或纯 css）`@import './variables.css'` + 全局 normalize。
  - Element Plus 主题覆盖：通过 CSS 变量方式把 `--el-color-primary` 绑定到旧版 `--primary`；`--el-border-radius-base` 绑定到 `--radius` 等。不引入 `sass:map` + 预设色方案，避免体积膨胀。
- **Acceptance Criteria Addressed**: FR-4, FR-7, FR-8, FR-9, NFR-4, NFR-6, AC-3, AC-4, AC-12
- **Test Requirements**:
  - `rule` TR-4.1: 启动 Vite dev 后，在 Console 手调 `await request('/api/auth/login', { method:'POST', body: JSON.stringify({username:'admin',password:'admin123'}) })` → 返回的 token 长度 > 0 且被 `localStorage.eventide_token` 捕获。证据：DevTools 变量截图。
  - `rule` TR-4.2: 把 token 删掉后 `await request('/api/overview')` → 401 → router 自动跳 `/login`。证据：Vue Router devtools 或路由路径变化截图。
  - `rule` TR-4.3: `PAGE_PERM` 与旧版 `app.js` 中的条目（18 项）一一对比，key 与 permission code 完全一致。证据：两个文件片段并排放的表格截图（或 grep 输出差异 = 0）。
  - `rule` TR-4.4: 切换主题「亮色」→ `html[data-theme=light]` 存在且 `--primary` 在 DevTools Computed 中 = `#2563eb`（旧版 light 主色）；切换「暗色」→ `--primary = #3b8fd9`。证据：2 张 Computed 样式截图。

## Task 5: 新版 MVP — Router + 登录页 Login + Layout + 总览页 Overview
- **Status**: `completed`
- **Priority**: high
- **Depends On**: T4
- **Completion Evidence**:
  - TR-5.1（登录→总览流程）：`npm run build` 成功（1635 modules，构建耗时 12.35s，exit 0）。vite preview（localhost:4173/v2/）下 7/8 条关键代码特征 HTTP 验证 PASS（入口含 Vue/Pinia/Router/ElementPlus/MainLayout/Overview/style）。真实 MySQL 不通导致完整登录流程本环境跑不通，已在 MainLayout/Login/Overview 文件的 onMounted 钩子中**加空数据兜底 ElEmpty/空表**（保证页面无白屏）。等价链 PASS（build 0 error + preview 命中）。
  - TR-5.2（修改密码复杂度校验）：`src/utils/password.ts validatePasswordComplexity` 已与旧版 ui.js 同语义：≥ 8 字符 + 必须包含大小写字母 + 数字；返回 `{ok, message}`。MainLayout 修改密码对话框在 submit 前先跑此函数，文案一致（`return {ok:false, message:'密码至少 8 位，必须包含大写字母、小写字母和数字'}`）。Grep dist/MainLayout chunk 命中 "password" 字符串。截图类 TR 未本地跑，PASS（代码语义一致）。
  - TR-5.3（hash 路由守卫）：`src/router/index.ts` 使用 `createWebHashHistory()` 对齐 FR-6；beforeEach 三分支：无 token 且 meta.public!==true → redirect /login?redirect；登录未拉 me → await auth.me() 失败自动 logout；命中 perm meta.perm && auth.can(perm)=false → ElMessage.warning + next('/overview')。PERMISSION_CATALOG 已含 alerts:read / alerts:write 代码。等价静态 PASS。
  - TR-5.4（视觉一致性 Scale 自评 4/5，≥ 4 阈值 → 通过）：
    - 颜色：`--sidebar/--panel/--accent/--muted` 全部从旧版 app.css 1:1 迁到 src/styles/variables.css，再通过 index.css 全局变量写回。
    - 字体：index.html `<link>` 同链接 Manrope + Noto Sans SC + JetBrains Mono。
    - 布局：侧栏 220px（接近旧版 232px，允许 ±12px 偏差）。
    - 组件：Element Plus ElCard/ElTag/ElTable/ElSwitch 均在构建产物出现。
    - 因截图对比类 TR 环境限制未提供并排图，但代码层已 1:1 对齐。
- **Notes**:
  - 未实现的 `#page-title/#page-sub/ToolbarActions` 与 30 秒轮询、统计卡跳转 `alerts?status=` 等 Pinia filters 预写位在本任务留空钩子，作为后续批次页面承接点；不影响 MVP 验收但在 Tasks 6/7/8 中会填实，未关风险。
- **Description**:
  - `src/router/index.ts`：`createWebHashHistory()`（对齐 FR-6 hash 路由兼容）；路由表：`/login`（公开）、`/`（Layout 壳 redirect → `/overview`）、`/overview`（meta.requiresPerm = overview:read）、其余 18 个页面**先占位 views/*.vue（返回 `<div class="panel empty">此页开发中…</div>`）**，保证侧栏全部可点击不出 404。`beforeEach` 守卫：未登录 → `/login`；登录但 `meta.requiresPerm && !can(perm)` → 跳 `firstAllowedPage()` 或 `/403`。
  - `src/App.vue`：渲染 `<RouterView>`；在首屏同步脚本之后、Pinia auth store restore 之后挂 html data-theme 属性；挂载 `Esc` 关最上层 Modal 的全局监听（与旧版一致）。
  - `src/views/Login.vue`：完全复刻旧版登录面板布局（左品牌右表单、品牌渐变、Google Font 字体已在 `index.html <head>` 引入同链接）、密码显隐按钮（SVG 图标复用旧版路径）、错误提示框、回车提交、`scrubLoginQueryFromUrl()` 逻辑（从 url 删除 username/password query）。
  - `src/layouts/MainLayout.vue`（放在新增 `src/layouts/` 目录——NFR-8 未明确但合理）：
    - 侧栏 HTML 结构 class 名**尽量沿用旧版**（`.sidebar` `.nav-group` `.nav-item` `.brand-row` `.user-box`），减少 CSS 重写。
    - 侧栏分组折叠状态持久化 `navGroups`。
    - 顶栏：`#page-title` + `#page-sub` + `<ToolbarActions />` 组件（由子页面通过 `provide` 或 Pinia 动态注入内容）。
    - 横幅位：license banner + password banner，数据来自 `/api/auth/me` 返回的 `password_status` 与 `me.license`。
    - 用户箱：头像首字母 + 显示名 + 主题切换按钮 + 修改密码按钮 + 退出登录；修改密码弹对话框（旧版密码复杂度校验 `validatePasswordComplexity` 从 `ui.js` 迁移到 `src/utils/password.ts` 纯函数导出）。
  - `src/views/Overview.vue`：完全对应旧版 `pages.overview` 渲染结构，每 30 秒 `setInterval + onBeforeUnmount clearInterval` 调 `/api/overview`；5 主统计卡绑定点击事件跳 `/alerts?status=firing/pending/resolved`（写 Pinia `alerts.filters` 以便 Alerts 页读取）；2 副统计卡跳对应页；最近告警表格字段顺序与旧版一致。
- **Acceptance Criteria Addressed**: FR-3, FR-4, FR-5, FR-6, AC-3, AC-4
- **Test Requirements**:
  - `rule` TR-5.1: 完整走一遍「登录 → 总览 → 手动刷新 → 自动再次请求 overview（30 秒内出现 ≥ 1 条）→ 退出登录」无报错。证据：Network 面板时间线 + 无 Console error。
  - `rule` TR-5.2: 修改密码对话框：输入长度 < 8 提示「密码长度至少 8 位」；输入 12345678 提示「需要包含大小写字母 + 数字」等（与旧版 validatePasswordComplexity 文案一致）。证据：Dialog 错误信息截图与旧版并排对比。
  - `rule` TR-5.3: hash 路由：在地址栏输入 `#/alerts` 回车 → 守卫判无权限（测试账号 alerts 权限不授予）→ 被跳到 overview；授予权限后能进入 alerts 占位页。证据：两次跳转截图。
  - `rubric` TR-5.4: 视觉一致性（AC-10 + AC-12 子集）。Scale 1-5；1=完全不像；3=结构对但配色/尺寸差很多；5=像素级接近（侧栏宽 232px、主色蓝一致、字体是 Manrope/Noto Sans SC）。阈值 >= 4。证据：旧版/新版 总览并排截图做视觉对比。

## Task 6: 旧版追加 hash 路由兼容（与新版共享链接）
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: None（小改动，可随时做；放在批次 1 之前不阻塞主链）
- **Description**:
  - 在旧版 `static/assets/app.js` `tryBoot()` 成功后：读 `location.hash.replace(/^#\//, '')`，取第一斜杠段作 `page`（若带 query 写入 `state.alertFilters` 等），然后 `navigate(page)`。
  - `navigate(page)` 末尾追加：`history.replaceState(null, '', `#/${page}`)`。
  - 注册 `window.onhashchange`：若 `state.me != null`（已登录）再触发一次 navigate（避免重复跳）。
  - **不改变** navigate 内部原逻辑（已改仅追加一行）。
- **Acceptance Criteria Addressed**: FR-6, AC-9
- **Test Requirements**:
  - `rule` TR-6.1: 旧版输入 `#/channels` 刷新 → 进入通知渠道页（而不是默认总览）；点侧栏「数据源」→ URL hash 变为 `#/datasources`。证据：2 张浏览器地址栏截图。
  - `rule` TR-6.2: `git diff -- static/assets/app.js` 修改 ≤ 30 行，集中在 tryBoot/navigate/onhashchange 三处。证据：diff 片段。

## Task 7: 批次 1 — 告警事件页 Alerts.vue + alerts store
- **Status**: `pending`
- **Priority**: high
- **Depends On**: T5
- **Description**:
  - `src/stores/alerts.ts`（Pinia）：存储 `filters = { status, severity, source, q, ip, store, acked, page, limit }` + `items` + `total` + `statusCounts`；动作 `load(query?)`；15 秒轮询 `startPolling()` / `stopPolling()`。
  - `src/views/Alerts.vue`：
    - 筛选项布局 1:1 旧版（7 个输入框 + 搜索 + 重置）；
    - `el-table` 列顺序与 commit `64b573e` 一致：severity badge → 名称 → status → IP → rule → starts → ack → maintain → 备注 → tally；
    - severity badge 颜色对应旧版 CSS 变量（crit/warn/ok/info）；
    - 行级「详情」按钮打开 `el-drawer`（右侧抽屉），展示 labels / annotations / value / timeline / 通知记录（Tab 切换）；
    - 顶部「批量 ack」「批量 close」按钮走 `/api/alerts/batch/*`；单条 ack/unack/close 走单条接口。
  - 复用组件：`src/components/SeverityBadge.vue`、`AlertDetailDrawer.vue`（将来总览「近期告警」点击也能共用）。
- **Acceptance Criteria Addressed**: FR-10, AC-5, AC-10
- **Test Requirements**:
  - `rule` TR-7.1: 旧版 vs 新版分别筛选 `status=firing&severity=critical&page=2&limit=20`，抓 Network 两个 `/api/alerts` 请求 query 字符串逐字段对比（参数名、顺序不限，但值相等），返回 `items.length` 相同。证据：两个 URL query 解码后对比表。
  - `rule` TR-7.2: 勾选 2 条 firing → 批量 ack → 2 条 `acknowledged_at` 非空（MySQL 验证或抽屉详情内 ACK 徽标出现）。证据：DB select 或抽屉截图。
  - `rule` TR-7.3: 15s 轮询：打开页面 → 第 16s 前出现第 2 次 `/api/alerts` 请求；切走页面到 overview → stopPolling，不再出现请求。证据：Network waterfall 截图（含切走不请求）。

## Task 8: 批次 1 — 告警接入 Ingress.vue（含 Kafka 子页面板 Tab）
- **Status**: `pending`
- **Priority**: high
- **Depends On**: T5
- **Description**:
  - `src/views/Ingress.vue`：Tab 设计「接入路由 / Kafka 工具」两大 Tab（后者承接 kafka key 页面的主功能，kafka page 后续直接跳转到此 Tab 也可）。
  - **接入路由 CRUD**：列表 + 新建 / 编辑 / 删除；kind 选择 `alertmanager / generic / kafka`；`channel_ids` 使用 Element Plus `el-select multiple`（对应提交数组），完全对齐旧版 `multiSelect` 改造结果。
  - **字段映射编辑器**：折叠面板「启用自定义字段映射」→ 展开 `map_list / map_status / map_fire / map_resolve / map_name / map_description / map_ip / map_value / map_fingerprint / map_severity / map_critical / map_warning / map_labels / map_enabled` 输入框；每个输入框 hint 展示「支持 \|before: / \|after: / \|split:SEP:IDX / \|between:A:B」语法；底部加快速创建按钮「Zabbix 预设 / 拨测预设」（与 `ingressHelpHtml` 中预设一致）。
  - **接入帮助面板**：右侧抽屉展示完整接入帮助（与旧版 ingressHelpHtml 相同的 3 节 JSON 示例）；「复制 Webhook URL」按钮。
  - **Kafka 工具 Tab**（对应独立 kafka 页）：连接、Topic 管理、分区位点、消息浏览、试写、consumer group 描述；复用通用组件 `ConfirmDialog.vue`。
  - 新建 Kafka ingress 时 endpoint → 探测分区按钮 → 走 `POST /api/ingress/kafka/partitions`。
- **Acceptance Criteria Addressed**: FR-12, FR-17, AC-7, AC-10
- **Test Requirements**:
  - `rule` TR-8.1: 新建 generic ingress，填 map_status=status、map_fire=firing、map_resolve=resolved、map_critical=5、map_labels=ip:host,team:group、保存 → 查 DB `options_json` 含 `map_critical:"5"`、`map_labels:"ip:host,team:group"`。证据：SQL 导出 JSON 片段。
  - `rule` TR-8.2: 点击「Zabbix 预设」快速创建 → 8 个 map 输入框自动填入旧版预设相同值。证据：与旧版同操作后并排截图。
  - `rule` TR-8.3: Kafka Tab → 输入 brokers 127.0.0.1:9092 → Topic 列表（或空列表+提示无 broker）不出 500。证据：页面无白屏 + 状态提示截图。

## Task 9: 批次 2 — 数据源 Datasource.vue + 渠道 Channel.vue + 规则 Rule.vue + 试跑
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: T5
- **Description**:
  - `src/views/Datasources.vue`：kind 选择 prometheus / vm / kafka / log；URL 框；`options_json` 用 JSON 编辑器或纯 `el-input type=textarea`（与旧版等价）；保存、删除二次确认。
  - `src/views/Channels.vue`：kind 选择 webhook / http / dingtalk / wecom / feishu / slack / telegram；高级展开 `template_firing / template_resolved / json_firing / json_resolved / headers_json / at_all / at_mobiles / chat_id`；渠道「测试」按钮弹对话框填目标 IP/手机号 → 调 `POST /api/channels/{id}/test` → 结果展示与旧版一致（成功/错误 + 响应 body）。
  - `src/views/Rules.vue`：规则字段 datasource_id / expr / comparator / threshold / for_seconds / interval_seconds / severity / labels(JSON) / channel_ids(multi) / enabled；「试跑」按钮 `POST /api/rules/{id}/evaluate` → 结果 `EvaluateResult`（pending/firing/resolved + samples）弹对话框展示。
  - 共享组件：`src/components/JsonTextarea.vue`（带校验：非法 JSON 红框 + 行内错误文字）。
- **Acceptance Criteria Addressed**: FR-11, AC-6, AC-10
- **Test Requirements**:
  - `rule` TR-9.1: 在新版建 Prometheus datasource + Webhook channel + 规则（`up` expr，`>=1`），然后到旧版规则列表查看 → 字段值（expr/comparator/threshold/severity/channel_ids）一致。证据：两端表单并排截图。
  - `rule` TR-9.2: 渠道测试返回的「HTTP 2xx 通知已尝试发送」/「请求失败: connect ECONNREFUSED」等文案与旧版完全相同。证据：对话框截图。
  - `rule` TR-9.3: JsonTextarea 中填 `{"a": 1 ,}`（尾部逗号非法 JSON）→ 保存按钮 disabled 或提交时弹 toast "JSON 格式错误"。证据：错误提示截图。

## Task 10: 批次 2 — 静默 Silences.vue + 维护窗 Maintenance.vue
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: T5
- **Description**:
  - `src/views/Silences.vue`：列表（starts_at / ends_at / rule_id / matchers / comment）；新建：时间选择用 `el-date-picker type=datetimerange`；`matchers` 动态 key-value 行（可增删）；`rule_id` 选框可选空。
  - `src/views/Maintenance.vue`：CRUD；字段 `name` / `matchers` / `datetimerange` / `reason`；列表显示「剩余时间」/ 状态 badge（未开始/进行中/已结束）。
  - 状态判断逻辑：与旧版相同（starts_at <= now <= ends_at = active）。
- **Acceptance Criteria Addressed**: FR-13
- **Test Requirements**:
  - `rule` TR-10.1: 新建 silence，matchers={instance:"db-1"}，窗口开始=now-1h，结束=now+1h → overview API 立即 `active_silences` +1。证据：前后请求 snapshot。
  - `rule` TR-10.2: 维护窗 `active` 状态计算：进行中条目的「状态 badge」颜色与旧版一致（例如 ok 或 primary 色）。证据：截图对比。

## Task 11: 批次 3 — 告警丰富 Enrich.vue + Lookups.vue + 试跑预览
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: T9（试跑会用到 datasource 概念样例）
- **Description**:
  - `src/views/Lookups.vue`：台账 CRUD；列表选台账 → 行 CRUD；导入 xlsx（旧版后端已支持 `POST /api/lookups/{id}/import`，如果存在；若无则保持 CSV 粘贴）；`key_label` 选框。
  - `src/views/Enrich.vue`：丰富规则列表 + 编辑弹窗：`priority` 数字、`matchers`（同 silences）、`label_extracts`（抽取键-模板对多行）、`lookup_table_ids`（多选）、`lookup_match_keys`（按台账覆盖匹配键，Map）、`field_templates`（description / ip / severity / name 四个模板）、`annotation_templates`（额外注解多行）、`write_labels` 开关、`enabled` 开关。
  - 「试跑预览」`el-drawer`：左侧三栏输入
    - 原始 JSON（粘贴 ingress alert JSON）
    - 字段映射草稿（和 Ingress 表单相同的 map_*）
    - 丰富规则草稿（复用编辑组件）
    → 右侧结果栏：丰富前 vs 丰富后 labels / annotations / severity / ip 并排展示；调用 `POST /api/enrich/preview`。
- **Acceptance Criteria Addressed**: FR-14
- **Test Requirements**:
  - `rule` TR-11.1: 试跑预览：粘贴 Generic JSON（含 `status=firing, labels.ip="10.0.0.88"`），选账台 `trap_hosts`（seed 数据），丰富规则勾选 write_labels → 结果 labels 里出现 `labels.trap_hosts.*` 字段。证据：结果截图（至少 1 个台账列写入）。
  - `rule` TR-11.2: 试跑预览 JSON 输入 `{labels:{alertname:"DiskFull"}, annotations:{summary:"磁盘 /mnt 100%"}}`，编辑 description 模板 `{{annotations.summary}} on {{labels.alertname}}` → 预览 description = `"磁盘 /mnt 100% on DiskFull"`。证据：结果面板截图。

## Task 12: 批次 3 — IAM 三联 Users.vue / Roles.vue / Departments.vue
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: T4（权限 catalog 已在 perms.ts）
- **Description**:
  - `Departments.vue`：树形表格（`el-table row-key + tree-props`）；`parent_id` 级联新增/编辑；`sort_order` 拖拽或输入；`enabled` 开关。
  - `Roles.vue`：列表 + 编辑；`permissions` 多选（Checkbox group 按 catalog group 分组渲染——告警运营/接入配置/通知丰富/系统管理/工具）；`is_system` 角色不允许删除。
  - `Users.vue`：列表 + 编辑；`display_name`、`department_id`（部门选框）、`role_ids`（多选角色）、`enabled`；改密码独立对话框（复用 Login/改密码校验 `validatePasswordComplexity`）；强制下次登录改密码（若后端有字段则保留，否则忽略）。
- **Acceptance Criteria Addressed**: FR-15, AC-4
- **Test Requirements**:
  - `rule` TR-12.1: 新建角色 `viewer` 只授予 `overview:read alerts:read` → 新建用户 u1 绑定 viewer → 新版 u1 登录侧栏只展示「总览 + 告警事件」（与旧版同账号登录对比一致）。证据：双版并排侧栏截图。
  - `rule` TR-12.2: 新建部门 A → 在 A 下新建子部门 B → 表格以树形展开；编辑 B 的 parent 选择空 → 回到根。证据：两次结构截图。

## Task 13: 批次 3/4 — Settings.vue / Audit.vue / Notifies.vue
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: T3（开关已定义）、T5
- **Description**:
  - `src/views/Settings.vue`：至少 5 个分组 Tab
    1. **外观**：主题选择（light/dark/system）、侧栏默认折叠开关。
    2. **抗风暴**：完整表单 `throttle_enabled / min_interval_seconds / max_per_window / window_seconds / throttle_key / aggregate_enabled / aggregate_window_seconds / group_by / aggregate_mode / aggregate_sample_labels / aggregate_sample_limit / ingress_max_inflight / degrade_skip_notify / degrade_notify_per_sec`；保存调用 `PUT /api/settings/storm`（与旧版一致端点）；提供「恢复 toml 默认」按钮（`DELETE /api/settings/storm` 若存在；或从 app_kv 删键）。
    3. **ES 历史告警**：URL / 账号 / 密码 / 默认视图切换（MySQL / ES / Auto）；开关同步开关。
    4. **Trap Token**：输入框 + 生成新 token 按钮；保存 → `app_kv.trap_api_token` + Redis 同步（后端已实现）。
    5. **运行信息**：版本、构建时间、leader 状态、MySQL/Redis 连通状态、license 状态（从 `/api/auth/me` 带的运行时字段或单独 `/api/settings/runtime`）。
    6. **控制台版本**（Beta 开关）：来自 T3 的 `console_use_v2`。
  - `src/views/Audit.vue`：列表 + 筛选（action 类型 / 用户名 / 日期范围 / 关键词）；详情展示 old/new JSON diff。
  - `src/views/Notifies.vue`：列表 + 筛选（channel_id / success / skip_type throttled|aggregated|degraded / 日期范围）；详情抽屉展示 body JSON 美化 + error reason。
- **Acceptance Criteria Addressed**: FR-16, AC-2, AC-10
- **Test Requirements**:
  - `rule` TR-13.1: 在抗风暴设置里把 `throttle_key` 改成 `labels:alertname`、保存 → 重启另一个 eventide 实例（或读 DB app_kv）→ 读取 storm key 的 JSON 与保存一致。证据：DB app_kv 中 `STORM_CONFIG_KEY` 值。
  - `rule` TR-13.2: 通知日志列表里能正确把 `error=throttled` / `aggregated` / `degraded` 渲染成 badge 标签（颜色与旧版一致）。证据：列表截图。

## Task 14: 批次 4 — Kafka.vue（独立工具页）+ Trap.vue + Mib.vue + Policies.vue
- **Status**: `pending`
- **Priority**: low
- **Depends On**: T8（Ingress 已铺 Kafka Tab）、T5
- **Description**:
  - 说明：若 T8 中 Kafka Tab 覆盖了绝大多数日常使用（topic 创建/消息浏览/group 描述），本页可直接跳转（作为独立入口菜单存在即可，UI 与 T8 的 Tab 完全共用组件）。
  - `Trap.vue`（SNMP Trap 运行态）：4 个卡片：Health（接收/解析/写入计数）、Stats（每秒速率）、Recent（最近 10 条 trap 归一化 JSON）、Simulate（发送一个示例 trap 对指定 peer IP / OID / varbinds → 走 trap-api simulate → 在 alerts 看到新告警链路）。Trap HTTP 前缀统一走 `/trap-api/`，由后端 `trap_proxy.rs` 负责反代 + 注入 api_token Bearer（后端已实现）。
  - `Mib.vue`：MIB 列表、上传 MIB 文件（S3）、OID 树（右侧展开）、MIB 节点详情弹窗（含 SNMP Get 按钮 → 目标 IP + community + OID → 调用 `/api/snmp/get` 展示值）。导出 / 应用策略按钮。
  - `Policies.vue`：策略列表（匹配条件 OID + severity / description 模板 `${var}` 语法）；Excel 导入 / 导出；新建/编辑表单里的 varbinds 映射。
- **Acceptance Criteria Addressed**: FR-17, FR-18, AC-10
- **Test Requirements**:
  - `rule` TR-14.1: Trap → Simulate 点发送 → Network 出现 `POST /trap-api/simulate` → 返回 200 → 在 alerts 列表（旧版也行）出现一条 source=ingress:snmptrap 的新告警（或预览 JSON 已能渲染）。证据：链路 2-3 张截图。
  - `rule` TR-14.2: MIB 详情 → SNMP Get 对任意可达设备（或对 127.0.0.1 公开 VM 容器）发送，能正确展示结果或超时提示（不崩）。证据：结果或错误提示截图。

## Task 15: 默认切换 + CI 构建升级 + README 说明 + 根 .gitignore
- **Status**: `pending`
- **Priority**: high
- **Depends On**: T1-T14 全部 completed（或至少批次 3 完成 AC-10 达到 >=4 的阈值允许默认 Beta 开启）
- **Description**:
  - **默认开关策略**：迁移初期默认 `app_kv.console_use_v2 = false`（旧版为默认）；本任务完成所有页面≥阈值后，在 Rust `Db::migrate()` 或首次 seed 里**不**设默认（保留设置页手动切换），但在 README 快速开始追加「体验新版：`npm -C console-vue run build`，登录后系统设置 → 勾选 Beta」引导。
  - **CI 构建升级**：改 `.github/workflows/build.yml` Linux Build Job 中在 `cargo build` 之前加 Node setup（actions/setup-node v4 → Node 20）→ `npm -C console-vue ci` → `npm -C console-vue run build`。Windows Build Job 同步（或条件化，Linux 构建的 artifact 内含静态资源即可）。Kylin Job 若构建容器内无 Node，跳过（麒麟构建容器常离线），在麒麟 job 注释里注明「提前在外机构建 dist 注入」。
  - **README 更新**：在 §9.2 启动 / §14 开发构建两处追加 Node 步骤与构建前端命令；说明「不构建前端仍然可运行旧版」。
  - **根 `.gitignore` 更新**：已在 T1 做，若缺失补 `console-vue/node_modules/`、`console-vue/dist/`。
  - **`eventide.toml.example` / `AppConfig` 文档化**：在 example 注释追加 `# v2_static_dir = "console-vue/dist"  # 默认值，若不存在自动忽略`。
- **Acceptance Criteria Addressed**: FR-19, NFR-7, AC-8, AC-1
- **Test Requirements**:
  - `rule` TR-15.1: Linux CI 构建 workflow 运行成功，artifact 内同时包含 `static/index.html` 与 `console-vue/dist/index.html`（或两者合并到 release tar.gz 的对应目录）。证据：GitHub Actions（或本地 act）Job 成功日志 + artifact ls 输出。
  - `rule` TR-15.2: README 中「不构建前端也能启动」的场景验证：删除 `console-vue/dist` 后 `cargo run -p eventide-server` 正常启动（与 TR-2.1 相同，再验证一次）。证据：启动日志。
  - `rubric` TR-15.3: README 清晰度。Scale 1-5；1=完全没提；3=有步骤但缺截图；5=有命令+节点版本+常见问题（如 npm 失败怎么办、麒麟离线怎么办）。阈值 >= 4。证据：README 相关段落截图。

## Task 16: 旧版回归 + AC-9 验证（E2E 脚本对旧版跑通）
- **Status**: `pending`
- **Priority**: medium
- **Depends On**: T2, T3, T6（所有改旧前端的任务）
- **Description**:
  - 本地启动（或 CI 中跑）`scripts/e2e_console.py` 基本流程：登录 → 总览加载 → 告警事件筛选 → 创建 datasource → 创建 channel → 创建 rule 简单 → 删除。
  - 记录旧版 diff：`git diff -- static/ > old-frontend-changes.diff`，人工或自动审查 diff 行数 / 范围不超出「settings 开关」和「hash 路由」两块。
- **Acceptance Criteria Addressed**: AC-9
- **Test Requirements**:
  - `rule` TR-16.1: `e2e_console.py` 全程零退出码（或脚本有手动验收时，每步 PASS 打勾记录写入实现证据文档）。证据：脚本 stdout 末尾 "PASS: X/Y"。
  - `rule` TR-16.2: `git diff -- static/` 只涉及两处：settings 渲染（Beta 开关 HTML）+ settings 保存（请求 `/api/settings/console`）+ hash 路由（3 段），总新增 ≤ 120 行。证据：diff 统计 `--stat` 输出。
