# Eventide 架构评审报告

> **重要提示**：本报告基于 GitHub `main` 分支代码（截至 2026-09）。用户本地 `D:\rustworkspace\eventide` 已有约 28 个本地提交 ahead of origin/main，包含 `scheduler.rs` 等文件的修改。以下发现可能与本地版本存在差异。

---

## 1. 系统概述与主要组件

### 1.1 系统定位

Eventide 是一个 **Rust 实现的轻量级告警引擎/运维平台**，定位为「团队级告警中枢」，支持：

- **主动拉数评估**：从 Prometheus/VM/Kafka/Loki 取数 → 阈值告警
- **被动告警接收**：HTTP Webhook / Kafka Topic → 汇聚通知
- 统一管线：告警标识去重 → 丰富 → 静默 → 多渠道通知

### 1.2 核心 Crate 架构

```
Cargo.toml (workspace)
├── crates/
│   ├── eventide-core/         # 领域模型、评估、fingerprint、ingress 解析、storm 节流/聚合
│   ├── eventide-notify/       # 多渠道通知：Webhook/DingTalk/WeCom/Feishu/Slack/Telegram/HTTP
│   ├── eventide-sources/      # 数据源适配：Prometheus/Kafka/Loki
│   ├── eventide-server/       # HTTP API + 调度 + 认证 + 主进程
│   ├── eventide-trap/         # SNMP Trap 独立服务
│   ├── eventide-trap-data/    # Trap MIB/Policy 共享存储层
│   └── eventide-license/      # Ed25519 离线许可证
├── vendor/samsa/              # 本地 patch 的 Kafka 客户端
├── console-vue/               # Vue3 影子前端（可选）
├── static/                    # 旧版静态控制台
└── deploy/trap-ha/            # Trap HA 部署示例
```

### 1.3 运行时组件

| 组件 | 归属进程 | 职责 |
|------|---------|------|
| HTTP Server (Axum) | eventide-server | API + 控制台静态资源 |
| Rule Scheduler | eventide-server | 定时规则评估（仅 Leader 执行） |
| Kafka Ingress | eventide-server | Consumer Group 消费告警流 |
| Notify Pipeline | eventide-server | 异步通知队列 + 限流 |
| Escalation Loop | eventide-server | 未确认告警升级扫描 |
| Aggregate Flusher | eventide-server | 聚合窗口到期刷新（仅 Leader） |
| SNMP Trap Receiver | eventide-trap | UDP 收包 → 解析 → Kafka |

---

## 2. 模块边界与耦合分析

### 2.1 清晰的边界（优点）

| 边界 | 说明 |
|------|------|
| `eventide-core` 无 I/O | 纯领域逻辑，可独立测试 |
| `eventide-notify` 单一职责 | 只负责各渠道的 HTTP 发送 |
| `eventide-trap` 独立进程 | 与主服务通过 Kafka + Redis 解耦 |
| `eventide-license` 独立签发 | 厂商侧工具，公钥编译进服务端 |

### 2.2 耦合风险

| 风险 | 位置 | 说明 |
|------|------|------|
| **AppState 巨型结构体** | `crates/eventide-server/src/state.rs` | 聚合了 18+ 个字段，所有 API handler 均依赖此单例 |
| **Db 结构体职责过重** | `crates/eventide-server/src/db/mod.rs` | 520+ 行 DDL + 迁移逻辑 + 全部 CRUD 混在一起 |
| **notify_pipeline 膨胀** | `crates/eventide-server/src/notify_pipeline.rs` | 612 行，混合了 persist、throttle、aggregate、escalate |
| **api/mod.rs 过长** | `crates/eventide-server/src/api/mod.rs` | 2660+ 行，建议进一步拆分为子模块 |

### 2.3 依赖图

```
eventide-server
 ├── eventide-core (领域)
 ├── eventide-sources (数据源)
 ├── eventide-notify (通知)
 └── eventide-trap-data (Trap 共享存储)

eventide-trap
 └── eventide-trap-data

eventide-trap-data
 └── (无 crate 内依赖，仅外部 mysql/redis/s3)
```

---

## 3. 安全与配置风险

### 3.1 高优先级（P0）

| 风险 | 文件 | 说明 | 建议 |
|------|------|------|------|
| **硬编码默认凭据** | `config.rs:213-218` | `admin/admin123`, JWT secret `eventide-dev-secret-change-me` | 生产部署必须修改；考虑首次启动强制重设 |
| **TOML 明文密码** | `eventide.toml`, `eventide-trap.toml` | MySQL/Redis/S3 密码明文 | 支持环境变量覆盖或 secret 文件 |
| **JWT 密钥过短/可预测** | `config.rs:219` | 默认 secret 可被猜测 | 文档强调 + 启动时检测弱 secret 告警 |
| **SHA-256 密码哈希无 KDF** | `password.rs:84-88` | 使用 `sha256$salt$hex`，无 bcrypt/argon2 | 迁移到 argon2/bcrypt（兼容旧哈希） |

### 3.2 中优先级（P1）

| 风险 | 文件 | 说明 |
|------|------|------|
| **CORS 全放开** | `main.rs:427` | `CorsLayer::permissive()` | 生产应配置可信 origin |
| **Trap API Token 可选** | `eventide-trap.toml` | `api_token` 为空时 HTTP 端点对外开放 | 添加启动警告 ✓（已有） |
| **Ingress Token 校验** | `ingress_api.rs` | HTTP 接入要求 ≥8 字符 token（已实现），Kafka 无 token | 文档说明网络隔离要求 |

### 3.3 配置模式观察

- **双 TOML 模式**：`eventide.toml` (主服务) + `eventide-trap.toml` (Trap 服务)
- **运行时覆盖**：`app_kv` 表存储控制台设置，热生效
- **Redis 同步**：Storm/Trap Token 配置经 Redis PUBLISH 多实例同步

---

## 4. 可靠性分析

### 4.1 调度器 (scheduler.rs)

**当前实现**：

```rust
// scheduler.rs:12-26
pub fn spawn_scheduler(state: Arc<AppState>) {
    let tick = state.config.scheduler_tick_seconds.max(1);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(tick));
        loop {
            interval.tick().await;
            if !state.leader.is_leader() {
                continue;
            }
            if let Err(e) = run_once(state.clone()).await {
                tracing::error!("scheduler tick failed: {e:#}");
            }
        }
    });
}
```

**风险**：
- ✓ Leader 选举避免重复评估（Redis lease）
- ⚠ `run_once` 失败后仅日志，无重试/告警
- ⚠ 并发信号量 `MAX_CONCURRENT_EVALS = 8` 硬编码

**建议**：
- 暴露配置项 `max_concurrent_evals`
- 考虑 circuit breaker 防止级联失败

### 4.2 Leader 选举 (leader.rs)

**优点**：
- Redis Lua 脚本原子续租
- 单实例模式可关闭 (`cluster.enabled = false`)

**风险**：
- Lease 过期时无优雅切换（下一个 tick 直接抢占）
- Leader 丢失后聚合窗口可能延迟刷新

### 4.3 Kafka Ingress (kafka_ingress.rs)

**优点**：
- Consumer Group 自动 rebalance
- 异步 notify queue 解耦落库与通知

**风险**：
- `seed_initial_offsets` 失败仅警告，可能导致重复消费
- 无消费积压监控（依赖外部工具）

### 4.4 Notify Pipeline (notify_pipeline.rs)

**优点**：
- 先 upsert 再 notify（at-least-once）
- 节流/聚合状态 Redis 共享

**风险**：
- 通知失败无自动重试队列
- 聚合窗口跨 Leader 切换可能丢失

---

## 5. 可维护性债务

### 5.1 代码质量

| 文件 | 行数 | 问题 |
|------|------|------|
| `db/mod.rs` | 520+ | DDL/迁移/CRUD 混合；缺少单独迁移系统 |
| `api/mod.rs` | 2660+ | 应按资源拆分子模块 |
| `main.rs` | 496 | 初始化逻辑过长 |
| `notify_pipeline.rs` | 612 | 职责过多 |

### 5.2 测试覆盖

| 区域 | 状态 |
|------|------|
| `eventide-core` | 有较完善的单测（storm、eval、fingerprint） |
| `eventide-server` | 集成测试缺失 |
| `eventide-trap` | 解析逻辑有测试，端到端覆盖不足 |

### 5.3 文档状态

- ✓ README.md 详尽（1400+ 行）
- ✓ 配置示例完整
- ⚠ API 无 OpenAPI/Swagger 规范
- ⚠ 架构图为纯文本

---

## 6. 优先级建议

### P0 — 安全（立即修复）

| ID | 问题 | 文件 | 建议 |
|----|------|------|------|
| P0-1 | 弱密码哈希 | `password.rs` | 迁移到 argon2/bcrypt |
| P0-2 | 默认凭据 | `config.rs` | 首次启动强制修改或生成随机 JWT secret |
| P0-3 | TOML 明文敏感值 | `*.toml` | 支持 `${ENV_VAR}` 占位符 |

### P1 — 可靠性

| ID | 问题 | 文件 | 建议 |
|----|------|------|------|
| P1-1 | 通知无重试 | `notify_pipeline.rs` | 添加 dead-letter 机制或定时重发 |
| P1-2 | Leader 切换时聚合丢失 | `storm_redis.rs` | 持久化窗口状态或减少窗口时长 |
| P1-3 | Scheduler 失败无告警 | `scheduler.rs` | 连续失败触发自身告警 |

### P2 — 可维护性

| ID | 问题 | 文件 | 建议 |
|----|------|------|------|
| P2-1 | DDL 与业务混合 | `db/mod.rs` | 引入 sqlx/refinery 迁移框架 |
| P2-2 | API 模块过大 | `api/mod.rs` | 按资源拆分为 `api/alerts.rs`, `api/rules.rs` 等 |
| P2-3 | AppState 膨胀 | `state.rs` | 拆分为 StorageState、NotifyState 等 |
| P2-4 | 缺少 OpenAPI | — | 添加 `utoipa` 或 `aide` 生成 |

---

## 7. 架构亮点

1. **Rust 类型安全**：领域模型在 `eventide-core` 集中定义，编译期保证类型正确
2. **Storm 设计**：节流 + 聚合 + 削峰三层防护，Redis 状态共享
3. **Trap 独立进程**：UDP 收包与主服务解耦，通过 Kafka 异步集成
4. **影子前端模式**：新旧控制台并存，Cookie 切换，低风险迁移
5. **离线许可证**：Ed25519 签名 + 安装绑定，适合私有化部署

---

## 8. 总结

Eventide 是一个功能完整、设计合理的告警平台。主要改进方向：

- **安全加固**：密码哈希升级、敏感配置外置
- **可靠性增强**：通知重试、聚合状态持久化
- **代码重构**：大文件拆分、迁移框架引入

建议按 P0 → P1 → P2 顺序逐步落地，避免一次性大规模重写。

---

*报告生成时间：2026-09-12*  
*审查范围：GitHub main 分支*
