# 贡献指南

感谢你对 Eventide 的关注！本文档说明如何参与开发与提交贡献。

## 快速开始

```bash
# 克隆仓库
git clone https://github.com/lvpenglive/eventide.git
cd eventide

# 拉起依赖（MySQL 8 + Redis 7）
docker compose up -d

# 复制示例配置
cp eventide.toml.example eventide.toml
cp eventide-trap.toml.example eventide-trap.toml
# 按需修改 mysql_url / redis_url

# 编译运行
cargo run -p eventide-server -- eventide.toml

# （可选）构建 Vue3 前端
npm --prefix console-vue ci
npm --prefix console-vue run build

# （可选）开发模式 HMR
npm --prefix console-vue run dev
```

默认账号：`admin` / `admin123`（**生产环境务必修改**）

## 仓库结构

```
eventide/
├── crates/
│   ├── eventide-core/      # 领域模型：评估、指纹、Ingress 解析、模板、丰富
│   ├── eventide-sources/   # 数据源适配：Prometheus / Loki / Kafka
│   ├── eventide-notify/    # 通知渠道：Webhook / 钉钉 / 企微 / 飞书 / Slack / Telegram
│   ├── eventide-server/    # HTTP API、调度、鉴权、控制台
│   ├── eventide-trap/      # SNMP Trap 独立服务
│   └── eventide-license/   # 许可证签发与验证工具
├── console-vue/            # Vue3 影子前端
├── static/                 # 旧版内嵌控制台
├── scripts/                # 辅助脚本（seed、e2e、截图）
├── deploy/                 # 部署示例
└── docs/                   # 文档与截图
```

## 开发规范

### Rust 后端

- **语言**：Rust 2021 edition，稳定版工具链
- **异步运行时**：Tokio
- **Web 框架**：Axum 0.8
- **错误处理**：使用 `Result<T, E>` 传播错误，**禁止生产代码中使用 `unwrap()`**（测试代码除外）
- **API 响应**：统一使用 `ApiResult<T>` / `ApiError`，通过 `map_err` 转换
- **状态管理**：`AppState` 使用 `Arc` + `RwLock` 保护可变状态，用 `unwrap_or_default` 安全访问
- **命名**：遵循 `rustfmt` 默认风格，模块名 `snake_case`，类型名 `PascalCase`

测试：

```bash
# 全 workspace 测试
cargo test --workspace

# 仅测试 core
cargo test -p eventide-core
```

### Vue3 前端

- **框架**：Vue 3 + TypeScript + Vite + Pinia + Element Plus
- **类型安全**：`tsconfig.json` 开启 `strict: true`，所有 API 响应有类型定义
- **API 封装**：统一通过 `src/api/request.ts` 发起请求，401 自动处理
- **状态管理**：Pinia store 管理全局状态（认证、配置等）
- **样式**：使用标准 CSS 变量（`--el-bg-color` 等），确保深色/浅色模式适配；不硬编码颜色值
- **UI 一致性**：参考 `static/assets/app.css` 中的类名约定（`.badge`、`.field`、`.row`、`.panel` 等）

开发：

```bash
npm --prefix console-vue run dev    # Vite dev server :5173
npm --prefix console-vue run build  # 产物到 console-vue/dist/
```

### 通用约定

- **不引入新的重量级依赖**，除非有充分理由
- **不破坏单二进制交付**的原则（前端构建产物随二进制一起发布）
- **不硬编码服务地址或凭据**，配置统一走 toml 文件
- **提交前**确保 `cargo test --workspace` 和 `npm run build` 均通过

## 提交规范

使用清晰的提交信息，建议遵循以下格式：

```
<类型>: <简述>

<可选详细说明>
```

类型包括：

| 类型 | 说明 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `refactor` | 重构（无功能变化） |
| `docs` | 文档更新 |
| `style` | 样式调整 |
| `test` | 测试补充 |
| `chore` | 构建、CI、依赖等杂项 |

示例：

```
feat: 支持 SNMPv3 authPriv 模式接收 Trap

fix: MIB 库页面 ElTree 懒加载根节点 moduleId 为空导致 400

refactor: 将 API shim 绑定逻辑抽取为公共工具函数
```

## Pull Request 流程

1. **Fork** 仓库并创建功能分支（`feat/xxx`、`fix/xxx`）
2. **开发**时保持分支聚焦，一个 PR 解决一个问题或实现一个功能
3. **测试**：确保以下全部通过
   ```bash
   cargo test --workspace
   cargo clippy --workspace -- -D warnings
   npm --prefix console-vue run build
   ```
4. **提交** PR，描述清楚改动内容和动机
5. **评审**通过后合并

## 报告 Bug

提交 Issue 时请包含：

- Eventide 版本（`eventide --version` 或 git commit hash）
- 操作系统
- 复现步骤
- 预期行为与实际行为
- 相关日志（去除敏感信息）

## 行为准则

请保持友好和尊重的交流态度。技术讨论对事不对人，不欢迎人身攻击或不当言论。
