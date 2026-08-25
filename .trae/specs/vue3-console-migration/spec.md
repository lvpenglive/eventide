# Eventide 控制台 Vue 3 迁移（影子前端 + 逐步切流）- 产品需求文档

## Overview
- **Summary**: 在不改动任何后端 API 与业务逻辑的前提下，新增一套基于 **Vue 3 + TypeScript + Vite + Pinia + Vue Router + Element Plus** 的「新版控制台」（影子前端），与现有 Vanilla JS 旧版共存；通过系统设置里的开关与 URL 前缀实现**无感切换**、**逐页迁移**、**一键回滚**，最终默认启用新版并保留旧版至少一个版本周期作为 fallback。
- **Purpose**: 解决旧前端 7700 行单文件 `app.js` 维护成本高、无类型安全、无组件化、无 HMR 热更新、开发体验差的结构性问题，为后续长期演进（图表看板、根因抑制、多租户等）打好工程化底座。
- **Target Users**:
  1. 开发者（你本人及后续协作者）— 获得 TS 类型、SFC 组件化、HMR、Pinia 响应式状态。
  2. 运维使用方 — 功能 1:1 兼容，可随时回滚；新版本逐步替换期间不阻断日常使用。

## Goals
1. 在仓库根新增独立构建目录 `console-vue/`，脚手架可一键 `npm install` / `npm run dev` / `npm run build`，构建产物输出到 `console-vue/dist/`。
2. Axum 后端新增「双静态目录」桥接能力：
   - 旧前端：`static/` 继续挂在 `/`（默认 fallback），**零行为变更**。
   - 新版前端：`console-vue/dist/` 挂在 `/v2/` 前缀；同时支持读取 Cookie/LocalStorage 里的「`eventide_use_v2=1`」开关，访问任意根路径时路由到新版 index（实现「设置切换后刷新即生效」）。
3. 新版前端完成基础 MVP：**登录页 → /api/auth/login → 主 Layout（侧栏 + 顶栏 + 用户菜单 + 主题切换 + 退出 + 改密码）→ 总览页**，打通 JWT、401 跳登、权限 `can()` 判定、明暗主题与旧版同套 CSS 变量。
4. 系统设置页（**旧版先加开关，新版同步实现**）新增「使用新版控制台（Beta）」开关，写 `app_kv`（服务端记忆）+ 前端写 LocalStorage/Cookie，下次刷新自动切流。
5. 按优先级**逐页迁移**旧版全部 17 个前端页面：alerts / datasources / rules / channels / ingress / silences / maintenance / enrich / lookups / users / roles / departments / settings / audit / notifies / kafka / trap + mib + policies（SNMP 三个页面），所有 CRUD、弹窗、试跑/预览、表格分页、筛选字段**行为与旧版一致**，后端接口 100% 复用。
6. 所有已迁移页面在新版中**前端权限可见性**与旧版完全一致（`PAGE_PERM` 映射表 + `perms.includes("*")` 超级管理员逻辑）。
7. 最后阶段：切换默认值为 `v2=true`，旧版静态目录保留一个版本（开关里加「使用经典版」回退按钮），CI 构建脚本增加 `npm -C console-vue run build` 步骤。

## Non-Goals
1. **不修改任何后端 `/api/*` 接口的请求/响应契约**（除非是「开关、版本标识这类纯前端切换元数据 API」，且必须显式新增独立端点不改旧契约）。
2. 不引入 SSR、Not Nuxt/Next、不改变「单二进制内嵌静态资源」的部署形态——构建产物仍以静态文件方式被 Axum ServeDir 直接托管。
3. 本次不做 UI 重新设计（Design Refresh）。新版视觉上 1:1 复刻旧版 slate/blue 主题（`--bg`、`--primary` 等 CSS 变量值保持一致），不在迁移中夹带换肤/换字体/换布局。
4. 不重写通知渠道 Webhook/钉钉/企微 等后端发送逻辑，不碰 Kafka Ingress / 规则调度 / 抗风暴等核心引擎代码。
5. 不引入微前端、不做 Module Federation、不用 Monorepo（pnpm workspace）；`console-vue/` 就是一个独立自包含的 Vite 项目，`package-lock.json`（或 `pnpm-lock.yaml`）与根 Cargo.lock 并列。
6. 旧前端 `static/assets/app.js` 不做进一步 ES module 拆分优化（那是 P0 路径浪费工作），只做必要的开关注入。
7. 本次不接入 Sentry/错误监控、不做 E2E 框架更换（现 `scripts/e2e_console.py` 后续可单独适配新版本）。

## Background & Context
- 项目现状：Rust workspace（eventide-core/server/notify/sources/trap 等 7 个 crate）+ `static/` 内嵌 Vanilla JS SPA；`app.js` ≈ 7700 行单文件（含 17 个页面 + 路由 + 状态 + UI 组件），仅 `js/pages/alerts.js` 拆出一个样例模块。
- 后端静态资源托管方式：Axum 0.8 `Router::new().merge(api_router).fallback_service(ServeDir::new(config.static_dir))`。`static_dir` 默认 `"static"`，见 [eventide-server/src/main.rs](file:///e:/eventide/crates/eventide-server/src/main.rs#L276-L279)。
- JWT 鉴权：`POST /api/auth/login` 入参 `{username, password}`，返回 `{token, username, expires_at, permissions:[...], password_status}`；JWT Claims 含 `uid`、`perms[]`，前端写 LocalStorage `eventide_token`，后续请求走 `Authorization: Bearer`，401 触发登出。见 [auth.rs](file:///e:/eventide/crates/eventide-server/src/auth.rs#L67-L75) 与 [api.js](file:///e:/eventide/static/assets/js/api.js#L16-L47)。
- 权限模型：权限码形如 `overview:read` / `alerts:read` / `rules:write` 等；超级管理员 `perms.includes("*")`；前端 `can(perm)` 语义 = `perms.includes("*") || perms.includes(perm)`；后端 `route_permission(method, path)` 按 HTTP method + path 判定。
- 主题：`html[data-theme="dark|light"]` 属性 + CSS 变量；`eventide_theme` LocalStorage 取值 `light|dark|system`；`data-theme-pref` 单独存模式偏好。
- 部署目标：`cargo build --release` 产出单二进制 + `static/` 目录。迁移完成后新部署流程 = `npm -C console-vue ci && npm -C console-vue run build` → `cargo build --release`，`eventide-server` 同时读 `static/` 与 `console-vue/dist/`。
- 选型依据：用户本人掌握 Vue 3 + TS，生态成熟度高；Element Plus 组件覆盖当前全部交互（表格/弹窗/表单校验/分页/抽屉/Tag）。

## Functional Requirements
- **FR-1（双前端桥接）**：Axum 路由支持同时托管旧版 (`static/`) 与新版 (`console-vue/dist/`)；`/v2/*` 始终返回新版 SPA；访问根路径 `/` 时若检测到 `eventide_use_v2=1`（Cookie 或 header 兜底），返回新版 index.html，否则返回旧版。
- **FR-2（切换开关）**：系统设置页新增「使用新版控制台（Beta）」布尔开关；保存时走 `app_kv`（复用现有 `[storm]` 同类设置写入机制），同时前端写 `eventide_use_v2` LocalStorage + 同名 Cookie（`path=/`；`SameSite=Lax`；可选 HttpOnly 关闭以便 JS 读）；用户下一次刷新即可切流；切换入口在旧版和新版设置页同时存在。
- **FR-3（新版 MVP 登录）**：新版提供登录页，用户名/密码、错误提示（登录失败次数过多、401 凭据错误）、密码显隐切换、明暗主题 4 项行为与旧版**像素级一致**；登录成功后写 JWT 到 LocalStorage `eventide_token`（与旧版相同 key，切换后免重登），同时写 `eventide_user` 用户名。
- **FR-4（新版 MVP Layout）**：新版 Layout 1:1 复刻旧版：
  - 左侧栏：品牌 Logo、侧栏折叠按钮（`eventide_sidebar_collapsed`）、5 个分组（告警运营/接入配置/通知丰富/系统管理/工具）、分组可折叠（`eventide_nav_groups`）、菜单项按权限渲染；
  - 顶栏：页面标题 + 副标题 + 工具栏 `#page-actions` 容器；
  - 用户箱：头像首字母、用户名、主题切换按钮（与旧版同位置 UI）、修改密码按钮、退出登录；
  - 许可证横幅 + 密码过期横幅展示逻辑与旧版一致。
- **FR-5（新版 MVP 总览页）**：新版总览页 `/overview`（Vue Router 路径）完整展示 `/api/overview` 所有字段：健康状态语、调度选主标签、HTTP 并发、firing/pending/resolved/静默/维护窗 5 个主统计（可点击跳告警）、启用规则/数据源/渠道/接入 4 个副统计、近期告警列表（≤10 条）、近期通知跳过记录、Ingress 清单；每 30 秒自动刷新 + 手动刷新按钮。
- **FR-6（路由与深链）**：新版 Vue Router 开启 `createWebHashHistory()`，旧版同时追加「hash 路由兼容」补丁（`navigate()` 时写 `#/page`，`load` 时读 `hash` → 首次进入的 page），保证新版和旧版刷新后都能跳到指定页，`/v2/#/alerts?status=firing` 这种分享链接直接生效。
- **FR-7（主题变量）**：新版**直接重用或等价复刻**旧版 CSS 变量全集（`--bg`…`--primary-border`、`--radius`…`--shadow` 等 40+ 个），`[data-theme=dark]` 与 `[data-theme=light]` 颜色值完全一致；Element Plus 组件主题用 CSS 变量覆盖策略，确保按钮/表格/弹窗样式观感与旧版一致（不使用默认 EP 蓝色，换成旧版 `#3b82f6 / #3b8fd9`）。
- **FR-8（API 客户端封装 / 全局拦截）**：
  - `src/api/request.ts` 封装 fetch（或 axios，选型在计划阶段锁定一个）：自动 `Authorization: Bearer`；HTTP 401 → `router.push('/login')` 清 token；HTTP 402 / body.code=license_readonly → 统一弹只读宽限提示；`throw Error` 时 `message` 与旧版 `api.js` 保持相同文案。
  - 所有 API 调用统一导出 TypeScript interface（至少请求/响应两类），放在 `src/api/types.ts`。
- **FR-9（前端权限）**：`src/perms.ts` 里 `PAGE_PERM`、`PERMISSION_CATALOG`、`can(perm)` 函数与旧版**同表同语义**；菜单项渲染、路由 `meta.requiresPerm` 守卫 2 层同时生效，防止手输 URL 跳无权限页。
- **FR-10（告警事件页 - 批次 1 高优）**：新版 `/alerts` 完整复刻旧版 alerts.js：
  - 状态/级别/来源/关键词/IP/存储/已确认 7 个筛选框 + 分页（默认 ALERT_PAGE_SIZE = 20）；
  - 表格列：severity badge / 名称 / status / IP / rule / starts（开始时间）/ ack / 维护 / 备注 / 计数（tally）；顺序与近期 commit `64b573e` 保持一致；
  - 单条 ack/unack/close/批量 ack/批量 close / 详情抽屉（labels + annotations + 通知记录）全部可用；
  - 每 15 秒轮询刷新（与旧版 `alertsMod.startAlertsTimer` 一致）并保留用户筛选。
- **FR-11（数据源/规则/渠道 - 批次 2）**：新版 `/datasources`、`/rules`、`/channels` 三页 CRUD + 列表 + 新建/编辑弹窗 + 试跑（规则 evaluate / 渠道 test）/ 删除二次确认与旧版等价；渠道 `channel_ids` 在接入表单是**多选**（对应提交 `string[]`，来自 commit `64b573e`）。
- **FR-12（告警接入 ingress - 批次 1）**：新版 `/ingress` 复刻接入路由 CRUD、Kafka 分区探测、Topic 管理、消息浏览/写入/consumer group 描述等工具子页；字段映射表单（`map_list`/`map_status`/`map_fire`/`map_resolve`/`map_name`/`map_description`/`map_ip`/`map_value`/`map_fingerprint`/`map_severity`/`map_critical`/`map_warning`/`map_labels`/`map_enabled`）与接入帮助说明（`ingressHelpHtml`）完整还原，并包含快速创建按钮。
- **FR-13（静默/维护窗 - 批次 2）**：新版 `/silences` + `/maintenance`：时间窗选择器（UTC/本地时区保持旧版行为）、`matchers` 标签匹配、`rule_id` 可选绑定、列表状态与剩余时间标签与旧版一致。
- **FR-14（告警丰富 + 台账 + 试跑预览 - 批次 3）**：新版 `/enrich` + `/lookups`：
  - 台账：导入 / 导出、匹配键 `key_label`、行级 CRUD；
  - 丰富规则：`matchers` + `priority` + `label_extracts` + `lookup_match_keys` + `field_templates` + 注解模板 + 写回开关；
  - 「试跑预览」弹窗：左侧输入 JSON + 接入映射草稿 + 丰富规则草稿，右侧分栏展示丰富前后 labels / annotations / summary / severity / ip 等字段，接口复用 `POST /api/enrich/preview`。
- **FR-15（用户/角色/部门 IAM - 批次 3）**：新版 `/users`、`/roles`、`/departments`：部门树（parent_id 级联）、角色绑定权限（多选权限码目录）、用户绑定部门 + 角色 + 启停用；密码复杂性校验与旧版 `validatePasswordComplexity()` 相同（长度 + 字符类型）。
- **FR-16（系统设置 / 审计 / 通知日志 - 批次 3/4）**：
  - `/settings`：外观（主题）、抗风暴表单（throttle / aggregate / inflight degrade 全套字段，保存后走 app_kv + Redis PUB/SUB 热生效，与旧版完全相同后端路径）、ES 历史告警配置、Trap Token、运行信息；
  - `/audit`：审计日志列表 + 按操作/用户/时间筛选；
  - `/notifies`：通知发送记录列表，支持点详情查看 body/error（含 `throttled` / `aggregated` / `degraded` 类型标识）。
- **FR-17（Kafka 工具页 - 批次 4）**：新版 `/kafka`：连接配置 → Topic 列表/创建/删除 → 分区与位点 → 消息浏览（按分区/offset/time）→ 试写消息 → consumer group 列表/描述/lag 查看；接口全部复用 `/api/ingress/kafka/*`。
- **FR-18（SNMP Trap 三联页 - 批次 4）**：新版 `/trap`（运行态：health/stats/recent/simulate，走 `/trap-api/` 反代）+ `/mib`（MIB 库 CRUD、OID 树浏览、xlsx 导入导出、SNMP Get）+ `/policies`（Trap 策略 CRUD、Excel 导入导出、按 OID 匹配）。
- **FR-19（构建与 CI）**：根 `.github/workflows/build.yml` 在 Linux 构建 Job 中增加步骤：安装 Node 20 LTS → `npm -C console-vue ci` → `npm -C console-vue run build`；Windows/Linux 打包 artifact 中包含 `static/` 与 `console-vue/dist/` 两份静态资源；本地 `README.md` 快速开始章节追加「构建前端」步骤说明（在 cargo run 之前）。

## Non-Functional Requirements
- **NFR-1（API 兼容性）**：新版任意页面发起的 HTTP 请求 method/path/query/body **在语义与字段上完全等于**旧版同操作请求；可通过 `scripts/e2e_console.py` 或浏览器 DevTools HAR diff 验证。
- **NFR-2（可回滚性）**：任何时刻用户在设置页关闭 Beta 开关 → 刷新即回到旧版，JWT 与筛选/侧栏偏好（LocalStorage key 相同）继续生效；开关默认值永远由 `app_kv` 控制，部署环境可在上线前关闭以默认旧版。
- **NFR-3（构建产物体积）**：新版首屏 JS（经 Vite build + gzip）≤ 500 KB；单 chunk `index-*.js` ≤ 350 KB；通过 `vite.config.ts` 中 manualChunks 拆分（vue / vue-router / pinia / element-plus 独立 chunk），超过阈值需补齐拆分策略。
- **NFR-4（类型安全）**：Vue SFC 全部 `<script setup lang="ts">`；所有 `api()` 调用有显式 `Promise<T>` 返回类型；`tsconfig.json` `strict=true`；`vue-tsc --noEmit` 作为 CI 步骤，0 error。
- **NFR-5（开发体验）**：`npm run dev`（Vite 开发服务器 5173）通过 `server.proxy` 把 `/api`、`/trap-api` 反代到 `http://127.0.0.1:8080`（Rust 后端），HMR 生效；修改 TS/Vue 文件 ≤ 1.5s 热更新到浏览器；Vite 冷启动 ≤ 5s（桌面 16GB 内存机器）。
- **NFR-6（无障碍与视觉一致）**：明暗主题切换无白屏闪烁（旧版 HTML head 内 theme 初始化逻辑在新版 index.html 同样保留一段**同步阻塞的内联 script**，不放到 Vue mount 之后）。
- **NFR-7（静态资源零阻塞 Rust 启动）**：即使 `console-vue/dist/` 不存在（前端没构建），`cargo run -p eventide-server` 仍能正常启动并使用旧版 `static/`；只有访问 `/v2/*` 时返回 404 并在日志里 WARN，绝不导致 500。
- **NFR-8（代码组织）**：新版目录结构遵循标准 Vite + Vue 分层：
  ```
  console-vue/
  ├── index.html                 # 含 head 同步 theme 脚本
  ├── src/
  │   ├── main.ts                # app mount + router + pinia + EP
  │   ├── App.vue
  │   ├── router/index.ts        # 路由表 + meta.requiresPerm
  │   ├── stores/                # Pinia：auth / ui(主题/侧栏) / settings
  │   ├── api/                   # request.ts + 每个实体一个模块 + types.ts
  │   ├── views/                 # 页面 SFC（与 pages 同名）
  │   ├── components/            # 通用组件：表格、模态、密码输入、权限 chip
  │   ├── composables/           # usePermission、useTheme 等
  │   ├── perms.ts               # PAGE_PERM + can()
  │   ├── styles/                # index.scss + variables.css（复刻旧版变量）
  │   └── env.d.ts
  ├── vite.config.ts
  ├── tsconfig.json
  ├── tsconfig.node.json
  ├── package.json
  └── package-lock.json
  ```

## Constraints
- **技术**：
  - 前端框架：Vue 3.4+ `<script setup>`、TypeScript strict、Vite 5.x、Vue Router 4.x、Pinia 2.x、Element Plus 2.x。可选引入 `@vueuse/core` 但禁止引入 Lodash、Moment.js 这类大体积库（Day.js 或 `date.toLocaleString` 够用）。
  - 后端：Axum 0.8 已有 `ServeDir` 与 `fallback_service`；桥接必须在同一个 `main.rs` 路由装配点完成，不拆出独立 crate。
  - 部署：单二进制 + 静态目录。最终 release 构建机器上必须能执行 `npm run build`（Node 20+），且在 README 与 CI 中明确说明；**不允许把构建后的 `console-vue/dist/*.js` 提交进 Git**（`.gitignore` 必须包含）。
- **业务**：
  - 迁移期间旧前端「零行为回归」。任何对 `static/` 或共享后端 `/api/*` 的修改必须显式证明「旧版行为不变」。
  - 所有旧版字段映射 `map_*`、丰富模板 `{{labels.x|before:SEP}}` 语法、接入 Generic / Alertmanager / Probe 解析 **纯后端能力**，前端新版只负责原样提交配置，不引入新的解析逻辑。
- **依赖**：
  - 运行依赖：MySQL + Redis 保持不变；Kafka（可选）；S3（MIB 用，可选）。
  - 新增构建依赖：Node.js ≥ 20 LTS、npm ≥ 10（或 pnpm ≥ 8）；由开发者机器与 CI runner 提供。

## Assumptions
1. 用户机器 Windows 10/11 已装或可以装 Node 20 LTS（`winget install OpenJS.NodeJS.LTS` 即可）；若环境受限，CI 构建 + 产物拷贝路径同样可用。
2. Element Plus 组件（el-table / el-form / el-dialog / el-select / el-date-picker / el-drawer）能覆盖旧版全部 UI，不存在必须手写原生 DOM 才能还原的交互缺口。
3. 旧版所有侧栏菜单项 ≈ 17 个功能页已在 `PAGE_ORDER` 枚举清楚；迁移进度以该表打勾追踪。
4. 切换开关存储优先用 Cookie（以便 Axum 在返回 `/` index 时**服务端可读**决定新旧版），LocalStorage 仅为兼容旧版读取；Cookie 路径 `/`，不设过期即 Session Cookie；设置页写的时候双写。
5. 迁移总工期目标：4 周 ± 1 周（MVP 1 周 + 批次 1–4 共 2–3 周 + 默认切换 + 清理 1 周）。

## Open Questions
- [ ] **Q1**: 新版 `index.html` 在 SSR 不可行前提下，首屏防白屏闪烁的 theme 脚本**写 Cookie 读旧版 theme key** 的逻辑是否需要同时把 `eventide_use_v2` 放进 theme 脚本？（暂定：不需要，use_v2 由 Axum 服务端按 Cookie 值决定返回哪份 index，前端再二次同步 LocalStorage 即可。）
- [ ] **Q2**: 通知渠道正文模板编辑器里的「点选芯片拖拽插入描述」功能（旧版 `enrich` 页用了原生 drag/drop），新版是否用 Element Plus `el-tag` + `onDragStart` 手拖即可，无需引入第三方拖拽库？（暂定：是。）
- [ ] **Q3**: 构建工具锁文件用 `package-lock.json` 还是 `pnpm-lock.yaml`？（暂定：`npm` 更通用，减少工具依赖；但如果用户偏好 pnpm 可切换。）

---

## Acceptance Criteria

### AC-1：后端桥接与回滚
- **Type**: `rule`
- **Given**: 已构建前端产物到 `console-vue/dist/`，Rust 服务正常启动
- **When**: 分别发起以下请求
  1. `GET /`（无 Cookie）→ 返回旧版 `static/index.html`
  2. `GET /` 携带 Cookie `eventide_use_v2=1` → 返回新版 `console-vue/dist/index.html`
  3. `GET /v2/#/overview` → 返回新版 index.html，且 `/assets/index-*.js` 路径被 ServeDir 正确解析
  4. 即使删除 `console-vue/dist/` 目录后重启服务，`GET /` 仍 200 返回旧版，`GET /v2/` 返回 404 并打 WARN 日志
- **Pass Condition**: 4 个场景全部符合预期，且任一 `/api/*` 请求行为与迁移前字节级一致（可对比 curl 输出）
- **Evidence**: 4 组 curl 命令 + 响应状态码/部分 body 快照；Rust 日志截图显示 404 只警告不抛异常

### AC-2：Beta 开关双向生效
- **Type**: `rule`
- **Given**: 用户已登录旧版控制台
- **When**: 在系统设置里勾选「使用新版控制台（Beta）」并保存 → 刷新页面；然后在新版设置里取消勾选 → 再刷新
- **Then**: 1) `app_kv` 表存在 `console_use_v2 = true/false`；2) 浏览器存在同名 Cookie `eventide_use_v2=1/0` 且 `Path=/`；3) 两次刷新分别正确跳到新版/旧版；4) JWT token 保持同一 LocalStorage key，跨版本不重新登录
- **Pass Condition**: 全链路按步骤通过，回滚一次也通过
- **Evidence**: 设置页保存后的浏览器 Application → Cookies/LocalStorage 截图；`app_kv` SQL 查询结果；两版分别能看到同一会话登录态

### AC-3：新版 MVP 功能
- **Type**: `rule`
- **Given**: Rust 服务运行、种子 admin/admin123 账号存在
- **When**: 打开 `/v2/#/login`，输入错误密码 5 次（触发登录锁定）→ 等待解锁 → 输入正确凭据 → 进入总览页
- **Then**: 1) 错误 5 次后显示「登录失败次数过多，请 X 秒后再试」；2) 登录成功跳总览；3) 总览页显示健康标语、5 主统计 + 4 副统计、近期告警列表；4) 30 秒后 `network` 面板可见一次 `/api/overview` 自动请求；5) 点 5 主统计 firing 卡 → 跳到 `/alerts`（先返回 404 或空页可以接受——批次 1 完成前该页占位即可）；6) 切明暗主题 → `html[data-theme]` 属性改变，颜色对应旧版 CSS 变量值
- **Pass Condition**: 以上 6 条全部通过
- **Evidence**: 浏览器 DevTools 的 Network + Console + DOM 截图，覆盖错误锁定、主题切换、自动刷新请求

### AC-4：权限可见性一致性
- **Type**: `rule`
- **Given**: 创建两个账号：A 有 `overview:read + alerts:read`；B 为超级管理员 `*`
- **When**: 用 A、B 分别登录旧版和新版，对比侧栏显示的菜单项数量与顺序
- **Then**: 两账号在两版中可见菜单项完全一致（A 只看到总览+告警事件；B 看到全部）；手输 URL `/v2/#/datasources` 对 A 跳回首个有权限页或显示 403（由路由守卫拦截）
- **Pass Condition**: 旧版 vs 新版可见菜单一致；无权限 URL 访问被守卫拦截
- **Evidence**: 账号 A 在两个版本的侧栏截图；账号 A 访问 `/v2/#/datasources` 被拦截日志或跳转结果

### AC-5：告警事件页（批次 1 核心）行为等价
- **Type**: `rule`
- **Given**: MySQL 中存在 ≥ 5 条告警（含 firing / resolved、含 severity=critical/warning/info，含 acked/unacked）
- **When**: 旧版 vs 新版分别打开 `/alerts`，执行以下动作：筛选 status=firing → 筛选 severity=critical → 翻到第 2 页 → 勾选 2 条 → 批量 ack → 点击 1 条行查看详情抽屉 → 关闭
- **Then**: 两版筛选条件参数（query 字段名、分页 `page/limit` 值、`status_counts` 返回）完全相同；批量 ack 2 条后两版中 acked 状态同步；详情抽屉中 labels + annotations + 通知记录字段相同
- **Pass Condition**: 所有步骤请求参数一致，UI 状态同步一致
- **Evidence**: 两个版本侧 by side Network 面板截图（`/api/alerts?status=firing&severity=critical&page=2&...`）；批量 ack 前后 SQL `acknowledged_at` 对比；详情抽屉字段对比截图

### AC-6：批次 2（规则/渠道/数据源）表单等价
- **Type**: `rule`
- **Given**: 新建 Prometheus 数据源 + Webhook 渠道 + 一条规则（绑定渠道）的完整流程
- **When**: 在新版中按旧版同样字段创建上述对象，然后用旧版列表页查看
- **Then**: 旧版能看到且可编辑新建的 3 个对象，字段值与新版输入一致；规则「试跑」返回 `EvaluateResult` 字段展示和旧版一致；渠道「测试」成功时通知发送记录 `notify_logs` 一致
- **Pass Condition**: 3 个对象跨版本可见、字段一致；试跑 + 测试成功
- **Evidence**: 新建时新版表单填写截图；旧版列表 + 详情页截图；`notify_logs` 行中 `success=1` 且 body 模板渲染正确

### AC-7：接入 ingress 字段映射表单完整
- **Type**: `rule`
- **Given**: 新建 Kafka ingress 路由，字段映射全部启用
- **When**: 新版表单填写 map_status / map_fire / map_resolve / map_name / map_description / map_ip / map_critical / map_labels 共 8 项，保存
- **Then**: 1) 数据库 `ingress_routes.options_json` 中所有 map 字段与旧版保存时结构一致；2) 新版打开编辑表单，8 项回填与旧版编辑回填一致；3) 点击「快速创建」按钮能正确插入预设 Zabbix / 拨测 映射
- **Pass Condition**: options_json 字段 diff 为零；回填与快速创建均通过
- **Evidence**: SQL 导出 `options_json` vs 旧版同操作导出；快速创建预设插入后表单截图

### AC-8：类型安全与 CI 构建
- **Type**: `rule`
- **Given**: 新版代码已推送，CI 或本地执行 `npm -C console-vue run type-check`（即 `vue-tsc --noEmit`）与 `npm -C console-vue run build`
- **When**: 运行上述命令
- **Then**: 1) type-check 0 error（strict 模式）；2) build 成功，产物 `console-vue/dist/assets/*.js` 最大 gzip 后 ≤ 500KB（可用 `gzip -c <file> | wc -c` 验证）；3) CI Linux build workflow 中增加 Node 步骤后能成功产出含 dist 的 release artifact
- **Pass Condition**: 1) 2) 3) 全部通过
- **Evidence**: `vue-tsc` 输出最后一行 "Found 0 errors"；gzip 体积命令输出；CI workflow run 成功日志

### AC-9：旧版零行为回归
- **Type**: `rule`
- **Given**: 工作区代码包含迁移所有改动
- **When**: 对旧版 `static/` 下 HTML/CSS/JS 做 diff：1) `index.html` 仅在「body 末开关脚本」处有改动；2) `app.js` 只有设置页 Beta 开关注入相关新增；3) 未修改任何现有函数签名；4) 旧版启动后按 E2E 脚本 `scripts/e2e_console.py` 基本流程（登录 → 总览 → 告警列表筛选 → 创建 datasource）跑通
- **Pass Condition**: diff 范围符合预期；E2E 脚本无失败
- **Evidence**: `git diff -- static/` 输出；e2e 脚本执行完成截图或日志

### AC-10：页面覆盖度
- **Type**: `rubric`
- **Dimension**: 新版前端对旧版 17 个功能页的功能等价覆盖度
- **Scale**: 1-5
- **Anchors**:
  - 1 = 只有 MVP（登录/Layout/总览）；其余页面全是占位或 404
  - 3 = 完成 MVP + 批次 1 & 2（alerts / datasources / rules / channels / ingress / silences / maintenance），高优页全部可用，但批次 3/4 仍是占位
  - 4 = 除了 Trap/MIB/Policies 三联页之外，其余 14 个页面全部可用（覆盖大多数用户的日常使用）
  - 5 = 全部 17 个页面（含 Trap 三联与 Kafka 工具）1:1 功能等价，无占位页
- **Pass Threshold**: >= 4
- **Evidence**: 新版侧栏每个菜单点击后的页面功能演示短视频或截图清单（附每个页面的「主要动作已验证」打勾表）

### AC-11：开发体验评估
- **Type**: `rubric`
- **Dimension**: 新版工程化带来的开发体验相对旧版提升程度（从改代码 → 浏览器看到效果的迭代速度、类型安全对减少 bug 的帮助、组件复用性）
- **Scale**: 1-5
- **Anchors**:
  - 1 = 比旧版更慢：冷启动 > 10s、HMR 失效、类型错误很多导致开发阻塞
  - 3 = 有一定提升：HMR 可用但 > 3s；TS strict 下有 any 泛滥；组件抽取不足 10 个
  - 5 = 明显提升：冷启动 ≤ 5s、HMR ≤ 1.5s；无 any；可复用组件 ≥ 20 个（表格壳、表单壳、密码输入、权限 chip、severity badge、时间选择器包装、Modal confirm、空状态等）
- **Pass Threshold**: >= 4
- **Evidence**: 本地 Vite 启动 + 改 `.vue` 文件一次 HMR 计时录屏截图或 `--debug` 日志；`tsc` 无 `// @ts-ignore` / 无显式 `any`（可通过 `grep -rE "@ts-ignore|: any"` 验证）；`src/components/` 目录 ls 清单

### AC-12：代码组织规范性
- **Type**: `rubric`
- **Dimension**: 新版目录结构、命名、分层符合 Vite + Vue 社区通用规范；权限、API、路由、stores 之间边界清晰
- **Scale**: 1-5
- **Anchors**:
  - 1 = 全部页面堆在 `App.vue` 单文件（和旧版同样问题）
  - 3 = 按 NFR-8 分层了目录，但 API 调用散落在各 SFC 里无统一 request 封装；路由守卫缺失
  - 5 = 严格遵循 NFR-8 结构；`api/request.ts` 做了 401/402 拦截；`router.beforeEach` 有 `requiresPerm` 守卫；Pinia store 不直接调 DOM；组件 props 有 TS 类型
- **Pass Threshold**: >= 4
- **Evidence**: 目录树 `tree -L 3 console-vue/src` 输出；`request.ts`、`router/index.ts` 守卫代码片段；Pinia `auth.ts` 代码片段
