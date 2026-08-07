# Eventide

Eventide 是一款用 **Rust** 实现的轻量级多数据源告警引擎，实现上走**MySQL + Redis、可嵌入控制台**的路线（多实例可共享库与风暴状态）。

它同时支持两种告警来源：

1. **主动拉数评估**：从 Prometheus / VictoriaMetrics / Kafka / Loki(Log) 取数，按规则判断是否告警  
2. **被动接收告警**：通过 HTTP Webhook 或 Kafka Topic 接收外部平台已判定的告警  

两条路径汇入同一套 **告警标识去重 → 告警丰富（台账补字段）→ 静默 → 多通道通知** 流水线。

> License: MIT。参考 同类产品 的产品划分，代码为独立实现（同类产品 为 AGPL，请勿直接复制其源码）。

---

## 目录

- [1. 它解决什么问题](#1-它解决什么问题)
- [2. 技术栈](#2-技术栈)
- [3. 仓库与模块](#3-仓库与模块)
- [4. 整体架构](#4-整体架构)
- [5. 核心概念](#5-核心概念)
- [6. 数据流与状态机](#6-数据流与状态机)
- [7. 存储设计](#7-存储设计)
- [8. API 一览](#8-api-一览)
- [9. 快速开始](#9-快速开始)
- [10. 配置说明](#10-配置说明)
- [11. 使用指南](#11-使用指南)
  - [11.8 抗告警风暴](#118-抗告警风暴怎么用)
- [12. 容量与边界](#12-容量与边界)
- [13. 路线图](#13-路线图)
  - [13.1 抗告警风暴实现清单](#131-下一步优先抗告警风暴--开工清单)
  - [13.2 更后可演进](#132-更后可演进)
  - [13.3 SNMP Trap 接入](#133-snmp-trap-接入)
- [14. 开发与构建](#14-开发与构建)
  - [14.1 CI 多平台打包](#141-ci-多平台打包)
  - [14.2 控制台前端演进](#142-控制台前端演进)
  - [14.3 本地调试日志](#143-本地调试日志)
  - [14.4 控制台 E2E](#144-控制台-e2e)

---

## 1. 它解决什么问题

现代可观测体系里，指标在 Prometheus，日志在 Loki，告警可能来自 Alertmanager、云监控、APM。运维常遇到：

- 多套告警源，通知散落在钉钉 / 企微 / 飞书  
- 同一问题反复刷屏（缺少统一去重）  
- 想自己写阈值规则，又不想上完整 Alertmanager 生态  

Eventide 定位为 **团队级告警中枢**：

| 能力 | 说明 |
|------|------|
| 规则引擎 | 阈值比较 + `for` 持续时长 |
| 多数据源 | Prometheus、VictoriaMetrics、Kafka、Log(Loki) |
| 告警接入 | Alertmanager / Generic Webhook / Kafka；支持自定义字段映射 |
| 告警丰富 | 台账查表、标签抽取、模板改写描述 / IP / 级别 |
| 抗风暴 | 通知节流、时间窗聚合、接入 429 / 降级（`[storm]`，见 §11.8） |
| 去重 | 规则 + 标签生成的告警标识（fingerprint） |
| 静默 | 按规则 ID / 标签匹配 |
| 通知 | Webhook、自定义 HTTP JSON、钉钉、企微、飞书、Slack、Telegram |
| 控制台 | 登录、侧栏菜单、CRUD 配置 |

---

## 2. 技术栈

### 2.1 语言与运行时

| 项 | 选型 |
|----|------|
| 语言 | Rust 2021 |
| 异步 | Tokio |
| HTTP 服务 | Axum 0.8 |
| 中间件 | tower-http（CORS、静态文件、trace） |
| 配置 | TOML |
| 日志 | tracing + tracing-subscriber |

### 2.2 数据与集成

| 项 | 选型 |
|----|------|
| 数据库 | MySQL（`mysql` crate） |
| 缓存 / 风暴状态 | Redis（节流、聚合） |
| HTTP 客户端 | reqwest（rustls） |
| Kafka | rskafka（纯 Rust 客户端） |
| 鉴权 | JWT（jsonwebtoken） |
| 序列化 | serde / serde_json |

### 2.3 前端

内嵌静态控制台（**无独立 Node 构建**，随 `eventide` 二进制一起交付）：

- HTML + CSS + Vanilla JS SPA（ES modules）  
- 字体：Syne / Figtree  
- 登录态：`localStorage` 存 JWT  
- 演进计划见 [§14.2](#142-控制台前端演进)（先体验与模块化，有信号再上 Vue/React）

### 2.4 与 同类产品 的差异（技术视角）

| | 同类产品 | Eventide |
|--|------------|----------|
| 语言 | Go | Rust |
| 存储 | MySQL | MySQL |
| 缓存 / 选主 | Redis | Redis（风暴状态 + 调度选主） |
| 前端 | React + Ant Design | 内嵌静态页 |
| 部署 | 多容器常见 | 单二进制 + 配置文件 |

---

## 3. 仓库与模块

```
eventide/
├── Cargo.toml                 # workspace
├── eventide.toml              # 运行配置
├── static/                    # 控制台前端（ServeDir 直出）
│   ├── index.html
│   └── assets/
│       ├── app.css
│       ├── app.js             # 路由 + 其余页面
│       └── js/                # ES modules（api / ui / pages/*）
└── crates/
    ├── eventide-core/         # 模型、评估、告警标识、Ingress 归一化
    ├── eventide-sources/      # Prometheus / Loki / Kafka 适配
    ├── eventide-notify/       # Webhook / 钉钉 / 企微 / 飞书
    └── eventide-server/       # HTTP API、调度、鉴权、静态资源
```

### Crate 职责

| Crate | 职责 |
|-------|------|
| **eventide-core** | 领域模型；阈值评估；告警标识；状态机；Ingress 解析与字段映射；模板渲染；告警丰富 / 台账查表 |
| **eventide-sources** | `fetch_samples()`：按数据源类型拉指标样本 |
| **eventide-notify** | 统一 `Notifier::send`，按渠道发通知 |
| **eventide-server** | 进程入口、JWT 鉴权、MySQL、Redis 风暴状态、规则调度、Kafka Ingress 轮询、REST API、控制台 |

依赖关系：

```
eventide-server
 ├── eventide-core
 ├── eventide-sources → eventide-core
 └── eventide-notify  → eventide-core
```

---

## 4. 整体架构

```
                    ┌─────────────────────────────────────────┐
                    │              eventide (单进程)            │
                    │                                         │
  Prometheus ──────►│  sources          core                  │
  VictoriaMetrics ──►│  (拉数) ──────► (评估/告警标识/状态机)   │
  Loki/Log ────────►│                      │                  │
  Kafka(消息字段) ─►│                      ▼                  │
                    │                 MySQL                    │
                    │            Redis（风暴状态）              │
  Alertmanager ────►│  ingress API                             │
  Generic Webhook ─►│  kafka ingress ──► 同一 AlertEvent       │
  Kafka(告警总线) ─►│                      │                  │
                    │                      ▼                  │
                    │  enrich（台账/模板）→ silence → notify   │
                    │                      → 钉钉/企微/飞书    │
                    │                                         │
                    │  Axum API + 静态控制台                    │
                    └─────────────────────────────────────────┘
```

### 运行时组件

1. **HTTP 服务**：对外 API + 控制台页面（多实例均可）  
2. **规则调度器**：按 `scheduler_tick_seconds` 轮询到期规则（仅 cluster leader）  
3. **Kafka Ingress**：以 Kafka **consumer group** 消费告警 Topic（多实例自动 rebalance）  
4. **MySQL**：配置与告警事件持久化  
5. **Redis**：通知节流 / 聚合状态 + 调度选主（不再用于 Ingress 分区分配）  

Kafka 仅作为可选数据源/接入；无消息中间件强依赖。

---

## 5. 核心概念

### 5.1 数据源 vs Ingress（务必区分）

| | 数据源 Datasource | Ingress |
|--|-------------------|---------|
| 方向 | Eventide **主动拉** | 外部 **主动推** / Eventide 消费告警流 |
| 内容 | 原始指标 / 日志统计 / 消息字段等 | 已判定的告警（firing/resolved） |
| 谁写规则 | Eventide 规则引擎 | 外部平台已完成判定 |
| 典型 | Prometheus、VM、Kafka 消息字段、Loki LogQL | Alertmanager、云告警 Webhook、告警总线 Kafka |

**Kafka / Log 可以两边都有，但含义不同：**

- **数据源 Kafka**：把 Topic 当数据管道，解析消息 JSON 字段 → 阈值告警（可选 depth/count）  
- **Ingress Kafka**：Topic 里是告警 JSON → 只做汇聚通知  
- **数据源 Log**：对 Loki 跑 LogQL → 自己告警  
- 日志平台若已告警再推过来 → 用 HTTP Ingress，不必再做「Log Ingress」

### 5.2 规则 Rule

- 绑定一个数据源  
- `expr`：PromQL / LogQL / Kafka JSON 字段路径（覆盖数据源 `options.field`）  
- `comparator` + `threshold`：阈值比较  
- `for_seconds`：条件持续多久才进入 firing  
- `interval_seconds`：评估间隔  
- `channel_ids`：通知渠道列表  

### 5.3 告警事件 AlertEvent

| 状态 | 含义 |
|------|------|
| `pending` | 条件已满足，但未达到 `for` |
| `firing` | 正式告警中 |
| `resolved` | 已恢复 |

字段含告警标识、标签、注解、当前值、通知边沿标记（`notified_firing` / `notified_resolved`）。

### 5.4 告警标识（fingerprint）

相同标签组合会得到同一告警标识，重复触发时合并为同一条事件，而不是新建多条。

对 `rule_id + 有序 labels` 做 SHA-256，用于去重。  
Ingress 场景会再带上 `route_id` 前缀，避免不同接入互相覆盖。

### 5.5 静默 Silence

在时间窗内，按可选 `rule_id` + `matchers`（标签全匹配）抑制通知。

### 5.6 通知渠道

| kind | 说明 |
|------|------|
| `webhook` | POST 固定 Eventide JSON（结构化字段 + text） |
| `http` | **自定义 HTTP JSON**：可配报文模板、方法、请求头、Bearer |
| `dingtalk` | 钉钉机器人（可签名；text / markdown；可 @） |
| `wecom` | 企业微信机器人（text / markdown；text 可 @） |
| `feishu` | 飞书机器人（可签名；text / interactive 卡片） |
| `slack` | Slack Incoming Webhook（text / mrkdwn blocks） |
| `telegram` | Telegram Bot `sendMessage`（需 `options.chat_id`） |

仅在状态边沿（变为 firing / 变为 resolved）发送，避免刷屏。

渠道可配置 **通知正文模板**（`options.template_firing` / `template_resolved`，控制台编辑）。留空则用默认纯文本。变量与丰富规则相同，并额外支持：

| 变量 | 含义 |
|------|------|
| `{{title}}` | 默认标题（级别 + 规则名 + 阈值 + 当前值） |
| `{{transition}}` | 边沿：`firing` / `resolved` |
| `{{labels}}` / `{{annotations}}` | 全部标签 / 注解的 JSON |
| `{{labels.x}}` / `{{annotations.x}}` | 单个字段（可接 `\|before:` 等变换） |
| `{{severity}}` `{{status}}` `{{value}}` `{{fingerprint}}` `{{rule.name}}` | 内置字段 |

其它常用 `options`：

| 键 | 说明 |
|----|------|
| `msg_type` | `text`（默认）或 `markdown` |
| `at_all` | `1`/`true`：钉钉/企微 text 模式 @所有人 |
| `at_mobiles` | 逗号分隔手机号（钉钉/企微 text） |
| `chat_id` | Telegram 会话 ID（必填） |
| `json_firing` / `json_resolved` | 自定义 HTTP 渠道的 JSON 报文模板 |
| `http_method` | `POST`（默认）/ `PUT` / `PATCH` |
| `headers_json` | 自定义请求头，如 `{"X-Token":"abc"}` |

`http` 渠道示例模板：

```json
{
  "msg": {{annotations.summary|json}},
  "severity": {{severity|json}},
  "labels": {{labels}}
}
```

`|json` 会输出带引号的 JSON 字符串字面量，避免描述里的引号弄破报文。

### 5.7 告警丰富 Enrich 与台账 Lookup

在落库与发通知前，可对 `AlertEvent` 做统一丰富（拉数评估与 Ingress 共用同一管线）：

| 概念 | 说明 |
|------|------|
| **台账 LookupTable** | 键值对照表（如 IP → 主机名 / 联系人）；`key_label` 为默认匹配标签名 |
| **丰富规则 EnrichRule** | 可启用多条；按 `priority` 升序执行；可用 `matchers` 限定生效范围 |
| **查表前抽取** | `label_extracts`：用模板从 summary/labels 截出临时标签（如 `sss_ip`） |
| **查表匹配键** | 默认用台账 `key_label`；规则内可按台账覆盖为抽取标签名（`lookup_match_keys`） |
| **写回字段** | 台账列写入 `labels.台账名.列名`；再用 `field_templates` 改写描述 / IP / 级别 / 名称 |

**单条规则内顺序**：抽取 → 台账查表 → 内联映射 → 注解模板 → 字段模板。

接入绑定的通知渠道发出的内容，是**丰富之后**的告警。

---

## 6. 数据流与状态机

### 6.1 拉数评估链路

```
调度到期规则
  → fetch_samples(datasource, expr)
  → evaluate_rule（逐样本比较阈值，处理 for）
  → 缺失样本则 resolve 旧告警
  → enrich_alert（台账 / 模板）
  → 命中静默则跳过通知
  → 边沿通知 + upsert alert_events
```

### 6.2 评估状态迁移（摘要）

| 当前状态 | 条件 | 下一状态 | 通知边沿 |
|----------|------|----------|----------|
| 无事件 | 匹配且 for=0 | firing | BecameFiring |
| 无事件 | 匹配且 for>0 | pending | 无 |
| pending | 持续匹配超过 for | firing | BecameFiring |
| pending | 不再匹配 | resolved | 无 |
| firing | 仍匹配 | firing | 无 |
| firing | 不再匹配 | resolved | BecameResolved |
| resolved | 再次匹配 | pending/firing | 可能 BecameFiring |

### 6.3 Ingress 链路

```
HTTP POST / Kafka 消费
  → 解析为 IngressAlert[]（含可选字段映射）
  → apply_ingress（与已有 fingerprint 合并）
  → enrich_alert（台账 / 模板）
  → 静默 / 边沿通知 / 落库
```

---

## 7. 存储设计

默认使用 **MySQL**（`mysql_url`）持久化配置与告警；**Redis**（`redis_url`）保存抗风暴节流/聚合状态。

本地可用 `docker compose up -d` 拉起 MySQL 8 + Redis 7（见仓库 `docker-compose.yml`）。

主要表：

| 表 | 内容 |
|----|------|
| `datasources` | 数据源（含 `options_json`） |
| `rules` | 告警规则 |
| `notify_channels` | 通知渠道 |
| `alert_events` | 告警事件（`fingerprint` UNIQUE） |
| `silences` | 静默策略 |
| `ingress_routes` | 接入路由（含 endpoint / options） |
| `ingress_kafka_offsets` | （遗留）旧版 MySQL 位点表；现由 Kafka consumer group 管理位点 |
| `enrich_rules` | 告警丰富规则 |
| `lookup_tables` | 台账数据 |
| `notify_logs` | 通知发送记录 |
| `departments` / `roles` / `users` | 组织与 RBAC（可选） |
| `schema_meta` | 迁移版本 |

Schema 通过启动时 `migrate()` 一次性建到当前版本（含 `notify_logs.body`、渠道 `options_json` 等）。

SQLite 单机版快照标签：`sqlite-baseline`（迁移前保留）。

---

## 8. API 一览

### 8.1 公开接口（无需 JWT）

| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/health` | 健康检查 |
| POST | `/api/auth/login` | 登录，返回 JWT |
| POST | `/api/ingress/{id}/alertmanager` | Alertmanager Webhook |
| POST | `/api/ingress/{id}/generic` | 通用告警 Webhook |
| POST | `/api/ingress/{id}/push` | 按路由 kind 自动解析 |

Ingress HTTP 接口可用：

- `Authorization: Bearer <token>`  
- 或 `X-Eventide-Token: <token>`  

（对应路由上配置的 `token`；**HTTP 接入创建/更新时 Token 必填，至少 8 字符；未配置则推送返回 401。Kafka 接入不使用 Token。）

### 8.2 需登录接口

Header：`Authorization: Bearer <jwt>`

| 前缀 | 能力 |
|------|------|
| `/api/auth/me` | 当前用户与运行信息 |
| `/api/overview` | 总览统计 |
| `/api/datasources` | 数据源 CRUD |
| `/api/rules` | 规则 CRUD + `/evaluate` 试跑 |
| `/api/channels` | 通知渠道 CRUD |
| `/api/alerts` | 告警列表（`status` / `severity` / `source` / `q` / `ip` / `store` / `page` / `limit`；返回 `{ items, total, page, limit, status_counts }`） |
| `/api/license` | 产品许可：`GET` 状态；`PUT` 导入授权文件/JWT；`DELETE` 清除 |
| `/api/license/request` | `GET` 导出授权申请（可 `?customer=`） |
| `/api/silences` | 静默 CRUD |
| `/api/ingress` | 接入路由 CRUD + 试推送 |
| `/api/enrich` | 丰富规则 CRUD |
| `/api/enrich/preview` | 试跑预览（可带接入映射 + 草稿规则） |
| `/api/lookups` | 台账 CRUD |

---

## 9. 快速开始

### 9.1 环境要求

- Rust 稳定版（建议 1.75+）  
- **MySQL** 与 **Redis**（可用 `docker compose up -d`）  

### 9.2 启动

```bash
# 可选：本地依赖
docker compose up -d

# 在仓库根目录（先改 eventide.toml 里的 mysql_url / redis_url）
cargo run -p eventide-server -- eventide.toml
```

默认监听：`http://0.0.0.0:8080`  
控制台：浏览器打开 `http://127.0.0.1:8080`  
默认账号：`admin` / `admin123`（见配置，**上线务必修改**）

### 9.3 产品许可（试用 + 离线授权）

- **首次启动**无商业许可证时自动开启 **30 天试用**（可写配置）。
- **试用/商业证过期**后进入**只读宽限**：可登录查看，写操作返回 `402` / `license_readonly`；仍可导入授权文件。
- 推荐流程（离线文件）：

```
客户控制台「导出授权申请」.json
        │
        ▼  发给厂商
厂商：eventide-license issue --request … -o xxx.eventide-lic.json
        │
        ▼  发回客户
客户控制台「导入授权文件」
```

**客户侧**：系统设置 → 产品许可 → 填写单位名称 → **导出授权申请** → 把 JSON 发给厂商 → 收到授权文件后 **导入授权**。

**厂商侧签发**：

```bash
# 私钥默认 keys/license_private.pem，或环境变量 EVENTIDE_LICENSE_PRIVATE_KEY
cargo run -p eventide-license -- issue \
  --request eventide-license-request-xxxxxxxx.json \
  --days 365 \
  --sub "客户名称" \
  -o customer.eventide-lic.json

cargo run -p eventide-license -- verify customer.eventide-lic.json
```

授权文件为 JSON（含 Ed25519 JWT），公钥编译进服务端；私钥勿提交仓库。

### 9.4 最小闭环（拉 Prometheus）

1. 登录控制台  
2. **数据源** → 新建 `prometheus`，URL 如 `http://127.0.0.1:9090`  
3. **通知渠道** → 新建 `webhook` 或钉钉  
4. **告警规则** → 填写 PromQL、比较符、阈值，绑定渠道  
5. 点 **试跑**，或等待调度自动评估  
6. 在 **告警事件** 查看结果  

### 9.5 最小闭环（接 Alertmanager）

1. **通知渠道** 先建好  
2. **告警接入** → 类型 `alertmanager`，填 token 与 channel  
3. 复制 Webhook URL：`/api/ingress/{id}/alertmanager`  
4. 在 Alertmanager 配置 `webhook_configs`  

### 9.6 可选：台账丰富后再通知

1. **告警丰富 → 台账数据**：导入主机对照表（匹配键如 `ip`）  
2. **丰富规则**：勾选台账，描述写 `{{labels.台账名.主机名}}`  
3. 用「试跑预览」验证；真实通知内容为丰富后的告警  

---

## 10. 配置说明

文件：`eventide.toml`

```toml
listen = "0.0.0.0:8080"
mysql_url = "mysql://eventide:eventide@127.0.0.1:3306/eventide"
redis_url = "redis://127.0.0.1:6379/"
static_dir = "static"
scheduler_tick_seconds = 5

[auth]
username = "admin"
password = "admin123"
jwt_secret = "eventide-dev-secret-change-me"
token_ttl_hours = 24
# 登录防爆破（按 IP + 用户名分别计数；默认 15 分钟内失败 5 次锁定 5 分钟）
login_max_failures = 5
login_window_seconds = 900
login_lockout_seconds = 300
# 密码过期提醒（天）；0 = 关闭
password_max_age_days = 90
password_warn_days = 14

# 抗风暴见 §11.8；完整字段见仓库内 eventide.toml 示例
# [storm]
# ...

# 多实例：规则调度选主（Kafka Ingress 用 consumer group）
[cluster]
enabled = true
leader_key = "eventide:cluster:leader"
lease_seconds = 15
# 以下两项已废弃（保留兼容，运行时忽略）
ingress_partition_lease_seconds = 15
ingress_max_claim = 0
```

| 字段 | 含义 |
|------|------|
| `listen` | HTTP 监听地址 |
| `mysql_url` | MySQL 连接串（库不存在时会尝试自动创建） |
| `redis_url` | Redis 连接串（风暴节流/聚合状态 + 选主） |
| `static_dir` | 控制台静态资源目录 |
| `scheduler_tick_seconds` | 规则调度基准间隔 |
| `auth.*` | 登录账号、JWT 密钥与有效期 |
| `[storm]` | 通知节流 / 聚合 / 接入削峰的**默认值**；控制台「系统设置」可覆盖并热生效（见 [§11.8](#118-抗告警风暴怎么用)） |
| `[cluster]` | 多实例：仅 leader 跑规则调度 / 聚合 flush；**Kafka Ingress 用 consumer group 扩容**；HTTP 与 webhook 仍全员服务 |
| `[cluster].ingress_partition_lease_seconds` | **已废弃**（忽略） |
| `[cluster].ingress_max_claim` | **已废弃**（忽略） |
| `[elasticsearch]` | 可选预置 ES 连接；**控制台「系统设置」也可改地址/账号**，并开关同步与默认「最近/历史事件」 |

启动时可指定配置路径：

```bash
cargo run -p eventide-server -- /path/to/eventide.toml
```

---

## 11. 使用指南

### 11.1 控制台功能

| 菜单 | 功能 |
|------|------|
| 总览 | firing/pending 数量、资源计数、最近告警 |
| 数据源 | Prometheus / VM / Kafka / Log |
| 告警规则 | CRUD、试跑 |
| 通知渠道 | Webhook / 钉钉 / 企微 / 飞书 |
| 告警接入 | Alertmanager / Generic / Kafka；字段映射与帮助说明 |
| Kafka | 轻量管理：连接、Topic 创建/删除、分区位点、消息浏览、试写（侧栏「工具」） |
| SNMP Trap | Trap 服务状态、试推送、对接 Kafka Ingress |
| 告警丰富 | 台账数据 + 丰富规则；试跑预览弹窗 |
| 告警事件 | 按状态筛选、详情（含丰富后 labels） |
| 静默策略 | 时间窗 + 标签匹配 |
| 用户 / 部门 / 角色 | 账号与权限（按部署启用） |
| 系统设置 | 外观、抗风暴、告警历史 ES、Trap Token、运行信息 |

### 11.2 数据源配置示例

**Prometheus / VictoriaMetrics**

```json
{
  "name": "prom",
  "kind": "prometheus",
  "url": "http://127.0.0.1:9090",
  "options": {},
  "enabled": true
}
```

规则 `expr` 示例：`up == 0` 或任意返回向量的 PromQL。

**Kafka（数据源：消息管道 / 字段告警）**

```json
{
  "name": "kafka-orders",
  "kind": "kafka",
  "url": "127.0.0.1:9092",
  "options": {
    "topic": "orders",
    "mode": "field",
    "field": "latency_ms",
    "label_fields": "service,instance",
    "max_records": "100",
    "partitions": "8"
  },
  "enabled": true
}
```

消息示例：`{"latency_ms":820,"service":"api","instance":"1"}`  
规则 `expr` 填字段路径（可覆盖 `options.field`），例如 `latency_ms` 或 `metrics.p99`；阈值如 `> 500`。

- 默认 `mode=field`：消费近期消息，解析 JSON 数值字段  
- `label_fields`：从消息拷贝标签，用于分组与告警标识  
- 可选 `mode=depth` / `mode=count`：Topic 堆积或近期条数（运维指标）

**Log / Loki**

```json
{
  "name": "loki",
  "kind": "log",
  "url": "http://127.0.0.1:3100",
  "options": {},
  "enabled": true
}
```

规则 `expr` 示例：

```text
count_over_time({app="api"} |= "ERROR" [5m])
```

要求 LogQL 返回可解析为数值向量的结果。

### 11.3 规则示例

```json
{
  "name": "api-down",
  "datasource_id": "<uuid>",
  "expr": "up{job=\"api\"}",
  "comparator": "<",
  "threshold": 1,
  "for_seconds": 60,
  "interval_seconds": 30,
  "severity": "critical",
  "labels": {"team": "sre"},
  "channel_ids": ["<channel-uuid>"],
  "enabled": true
}
```

比较符支持：`>` `>=` `<` `<=` `==` `!=`（API 也可用 `gt`/`gte` 等）。

### 11.4 Ingress 示例

**Alertmanager**

- 控制台创建 `kind=alertmanager`  
- URL：`POST /api/ingress/{id}/alertmanager`  
- 控制台支持「试推送」一键验证接入 → 告警事件闭环（也可 `POST /api/ingress/{id}/test`，需登录 JWT）  

**Generic / 拨测 Probe**

```json
{
  "alerts": [
    {
      "status": "firing",
      "fingerprint": "optional",
      "labels": {"alertname": "DiskFull", "instance": "db-1"},
      "annotations": {"summary": "disk > 90%"},
      "severity": "warning",
      "value": 92.5
    }
  ]
}
```

也支持  业务拨测 `probe-alert` 单条 JSON（或整行日志，自动截取 `{...}`）：

```json
{
  "eventType": "fire",
  "eventTime": "2026-07-21 15:18:39",
  "messageId": "2077682656462446593",
  "resultFlag": "BAD",
  "retCode": "4001",
  "retMessage": "…",
  "retTimeMs": 12976,
  "bizname": "oaec",
  "bizchainName": "业务拨测-办公管理系统",
  "alertCategory": "business"
}
```

| 拨测字段 | Eventide |
|----------|----------|
| `eventType` fire/recover | status firing/resolved |
| `messageId` | 告警标识 fingerprint（同链 fire↔recover 对齐） |
| `bizchainName` 等 | labels.alertname |
| `retMessage` | annotations.summary |
| `retTimeMs` | value |
| `alertCategory=infra` / `retCode=10001` | severity critical |

推送：`POST /api/ingress/{id}/generic` 或 `/push`；Kafka Ingress Topic 消息体同样可识别。

**自定义字段映射（Generic / Kafka）**

在接入编辑「字段映射」中填写对方 JSON 点分路径（控制台有可展开的字段说明）。任一路径非空或勾选「启用自定义字段映射」即启用，并优先于内置 Generic/拨测解析。

| 配置项 | 作用 | 写入结果 |
|--------|------|----------|
| `map_list` | 告警数组路径，空=整条消息当一条 | — |
| `map_status` | 状态字段 | firing / resolved |
| `map_fire` | 视为触发的取值（逗号分隔） | — |
| `map_resolve` | 视为恢复的取值 | — |
| `map_name` | 告警名称 | `labels.alertname` |
| `map_description` | 告警描述 | `annotations.summary` / `description` |
| `map_ip` | 告警 IP | `labels.ip` / `alertIp` / `instance`（三者同步） |
| `map_value` | 当前值 | `value` |
| `map_fingerprint` | 去重标识 | `fingerprint` |
| `map_severity` | 级别原始值 | `labels.severity` + 引擎级别 |
| `map_critical` | 哪些取值算严重 | → Critical |
| `map_warning` | 警告取值列表（引擎支持；控制台无单独框） | → Warning |
| `map_labels` | 额外标签，`目标标签:源路径,...` | 对应 `labels.*` |
| `map_enabled` | 强制开启映射 | — |

路径支持字符串截取（与丰富模板相同）：

| 语法 | 说明 | 示例 |
|------|------|------|
| `\|before:SEP` | 分隔符之前 | `sourceciname\|before:_` → `82.12.161.32` |
| `\|after:SEP` | 分隔符之后 | `summary\|after:为：` |
| `\|split:SEP:INDEX` | 按分隔符拆分，取第 INDEX 段（从 0） | `x\|split:_:0` |
| `\|between:起点:终点` | 两段之间；起点/终点可空 | `summary\|between:为：: %` |

Zabbix 风格示例：`map_ip=sourceciname|before:_`，`map_name=sourcealertkey`，`map_description=summary`，`map_fingerprint=sourceidentifier`，`map_severity=sourceseverity`。

**Kafka Ingress（告警总线）**

```json
{
  "name": "alert-bus",
  "kind": "kafka",
  "endpoint": "127.0.0.1:9092",
  "options": {
    "topic": "alerts",
    "start": "latest",
    "partitions": "8",
    "group_id": "eventide-ingress-alerts"
  },
  "channel_ids": ["<channel-uuid>"],
  "enabled": true
}
```

消息体支持 Alertmanager JSON、Generic 批量/单条、Jeecg 拨测，以及带字段映射的自定义 JSON。  
位点由 Kafka **consumer group**（`__consumer_offsets`）管理；默认 `group_id = eventide-ingress-{route_id}`，可用 `options.group_id` 覆盖。新 group 无提交位点时按 `options.start`（`latest`/`earliest`）种子位点。  
多实例共用同一 `group_id` 时由 Kafka rebalance 分摊分区；**不依赖 Redis**。通知走 **异步队列**（先落库再发渠道）。  
订阅分区数优先从 Kafka **metadata 自动探测**；探测失败时回退到 `options.partitions`。Trap 侧 `kafka_partitions` 建议与 Topic 真实分区数一致。

### 11.5 告警丰富（台账 + 规则）

典型路径：

1. **台账数据**：建表，设匹配键（如 `ip`），导入行（键 → 主机名 / 联系人等列）  
2. **丰富规则**：勾选台账；需要时先「查表前抽取」；配置告警描述 / IP / 级别模板  
3. **试跑预览**：粘贴原始 JSON，选字段映射接入，看丰富前后 labels / 描述  

**模板变量**（可用于抽取、描述、IP、级别、名称）：

| 写法 | 含义 |
|------|------|
| `{{labels.xxx}}` | 告警标签（含接入映射写入的 `ip` / `alertname` 等） |
| `{{labels.台账名.列名}}` | 台账命中后的列（必须带台账名前缀） |
| `{{annotations.xxx}}` | 注解，常见 `summary` / `description` |
| `{{value}}` / `{{severity}}` / `{{status}}` / `{{fingerprint}}` | 内置字段 |
| `{{rule.name}}` | 当前丰富规则名 |

截取语法与接入映射相同：`|before:` / `|after:` / `|split:` / `|between:`。

**推荐配置**

- 接入已映射出 `labels.ip` → 台账「用标签」填 `ip` → 描述写 `{{labels.device_info_form.主机名}}`  
- IP 只在描述里：抽取 `sss_ip = {{annotations.summary|before:_}}`，台账「用标签」填 `sss_ip`  

控制台支持点选 / 拖拽标签芯片插入描述；保存前可用「试跑预览」。

注意：查表键必须与 labels 中的名字一致。若只有 `alertIp` 没有 `ip`，旧版本会查不到台账；当前字段映射会同时写入 `ip` / `alertIp` / `instance`。

### 11.6 静默示例

```json
{
  "comment": "夜间维护",
  "rule_id": null,
  "matchers": {"instance": "db-1"},
  "starts_at": "2026-07-23T16:00:00Z",
  "ends_at": "2026-07-23T18:00:00Z"
}
```

### 11.7 用 curl 登录并调用 API

```bash
TOKEN=$(curl -s -X POST http://127.0.0.1:8080/api/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | jq -r .token)

curl -s http://127.0.0.1:8080/api/overview \
  -H "Authorization: Bearer $TOKEN"
```

### 11.8 抗告警风暴怎么用

短时间涌入大量相似告警时，靠 **节流 → 聚合 → 接入削峰** 保护通知渠道与进程。

- **默认值**：`eventide.toml` 的 `[storm]`
- **运行时覆盖**：控制台 **系统设置 → 抗告警风暴**（写入 MySQL `app_kv`，**当前进程立即热生效**，无需重启）
- **恢复默认**：设置页「恢复 toml 默认」会删除覆盖，回退到 toml
- **多实例**：节流/聚合**状态**在 Redis 共享；配置经 Redis **SET + PUBLISH** 同步，其它节点秒级热加载（无需重启）。控制台任意实例打开设置都读 MySQL 中的共享覆盖。

#### 推荐默认（仓库 `eventide.toml` 已接近此配置）

```toml
[storm]
# —— 通知节流（P0）——
throttle_enabled = true
min_interval_seconds = 60      # 同一 key 最短间隔（秒）
max_per_window = 20            # 滑动窗口内最多发送条数
window_seconds = 60
# fingerprint = 按单条告警限流
# labels:alertname     = 同名告警共用额度（多主机风暴更有效）
# labels:alertname,ip  = 按名称+IP
throttle_key = "fingerprint"

# —— 时间窗聚合（P1）——
aggregate_enabled = true
aggregate_window_seconds = 30
group_by = "alertname"         # 可写 alertname,severity
aggregate_mode = "head+summary"  # 或 summary_only
aggregate_sample_labels = "ip,instance,alertIp"
aggregate_sample_limit = 10

# —— 接入削峰（P2）——
ingress_max_inflight = 100     # 并发接入上限；0=不限制
degrade_skip_notify = false    # true=高压时只落库不发通知
degrade_notify_per_sec = 50    # 通知尝试速率阈值（配合 degrade）
```

#### 各能力说明

| 能力 | 作用 | 告警事件 | 通知 |
|------|------|----------|------|
| **节流** | 同渠道+同 key 限制发送频率 | 正常入库 | 超限跳过，日志 `throttled` |
| **聚合** | 窗口内同 `group_by` 多条触发合成 | 仍按 fingerprint 多行 | 首条可立即发；窗口结束发摘要；中间记 `aggregated` |
| **削峰** | HTTP 接入并发过高 | — | 返回 **429** `ingress overloaded` |
| **降级** | `degrade_skip_notify=true` 且高压 | 正常入库 | 跳过发送，日志 `degraded` |

聚合模式：

- `head+summary`：窗口内第一条立刻通知，结束时若 count>1 再发一条 `[聚合告警]` 摘要  
- `summary_only`：窗口内不发单条，只在结束时发摘要  

#### 典型场景怎么配

| 场景 | 建议 |
|------|------|
| 单机反复抖动刷屏 | `throttle_key = "fingerprint"`，保持默认节流即可 |
| 上百台主机同一 `alertname` 同时炸 | `throttle_key = "labels:alertname"`，并打开 `aggregate_enabled` |
| Webhook 被压测打爆 | 调低 `ingress_max_inflight`；必要时 `degrade_skip_notify = true` |

#### 如何确认生效

1. 启动或控制台保存后日志可见：`storm throttle enabled` / `storm aggregate enabled` / `storm ingress pressure configured`（控制台保存后当前进程已热加载，不一定有新日志行）  
2. 通知记录（`notify_logs`）中：  
   - `error=throttled`：被节流  
   - `error=aggregated`：计入聚合窗口、未单发  
   - `error` 含 `aggregate:...`：摘要已发送  
   - `error=degraded`：高压降级跳过通知  
3. 控制台 **告警事件** 仍应按 fingerprint 逐条可见（聚合不合并库内事件）  
4. 压测接入：超限时 HTTP 状态码 **429**

#### 注意

- 节流/聚合状态在 **Redis**，多实例共享；进程重启不丢（按 key TTL 过期）。  
- Ingress 削峰（inflight / degrade）仍为**本进程**计数。  
- 实现细节与验收清单见 [§13.1](#131-下一步优先抗告警风暴--开工清单)。

---

## 12. 容量与边界

### 适合

- 单团队 / 业务线告警聚合  
- 规则数百、活跃告警万级以内  
- 单机部署、运维简单优先  

### 不适合（当前架构）

- 海量日志/指标长期存储（应仍在 Loki / Prometheus）  
- 超高 QPS 且无队列削峰的极端风暴——可先用 [§11.8](#118-抗告警风暴怎么用) 的节流/聚合/429；更重的削峰需外置 Kafka 等  

多实例时：HTTP / webhook 可水平扩展；**规则调度、聚合 flush** 由 Redis lease 选主；**Kafka Ingress** 用 consumer group 分摊（见 `[cluster]` / `options.group_id`）。

瓶颈主要在 **MySQL 写并发**、**Redis 往返** 与 **单进程算力**，不在 Rust 语言本身。

### 安全注意

- 修改默认 `password` / `jwt_secret`；可调 `[auth] login_max_failures` / `login_window_seconds` / `login_lockout_seconds` 防爆破  
- HTTP Ingress **必须**配置 `token`（≥8 字符）；Kafka 靠网络与 ACL  
- 控制台与 API 建议置于内网或反向代理 TLS 之后  

---

## 13. 路线图

已完成（相对 同类产品 精简对齐）：

- [x] Prometheus / VictoriaMetrics 拉数评估  
- [x] Kafka / Log 数据源  
- [x] Alertmanager / Generic / Kafka Ingress（含自定义字段映射与截取语法）  
- [x] 告警丰富：台账查表、标签抽取、字段模板、试跑预览  
- [x] 告警标识去重、静默、多通道通知（通知前先丰富）  
- [x] JWT 登录与管理控制台  
- [x] 抗风暴 P0：通知节流（`[storm]` + `ThrottleGate`）  
- [x] 抗风暴 P1：时间窗聚合通知（`AggregateBuffer` + summary flush）  
- [x] 抗风暴 P2：接入 inflight 429 + notify degrade  

### 13.1 抗告警风暴 —— 实现清单（开发用）

> **运维使用说明**请看 [§11.8](#118-抗告警风暴怎么用)。本节保留实现落点与验收，便于二次开发。  
> **目标**：短时间涌入大量相似告警时，进程不挂、渠道不被刷爆、值班收到的是可消化的摘要。  
> **原则**：接得住（削峰）→ 合得住（聚合）→ 发得住（节流）。指纹去重已有，风暴缺口在 **通知节流 + 时间窗聚合**；队列为可选项。  
> **主落点**：`crates/eventide-server/src/notify_pipeline.rs` 的 `persist_and_notify`（enrich → silence → **此处插入 storm 门闸** → notify → upsert）。拉数评估与 Ingress 都走此函数，一处改两边生效。

#### P0 — 通知节流（先做，改动小、立刻见效）

**行为**

- 维度：`(channel_id, throttle_key)`。默认 `throttle_key = fingerprint`；可选按标签模板生成（见配置）。  
- 规则：滑动窗口内同一 key 最多发 `max_per_window` 条；另设 `min_interval_seconds`（同 key 两次通知最短间隔）。  
- 被节流的边沿：**仍 upsert 告警状态**，但跳过 `notifier.send`；`notify_logs` 记一条 `success=false` 或新增 `skipped=throttled`（二选一，实现时统一；推荐单独字段/ error=`throttled` 便于统计）。  
- firing / resolved **分开计数**（避免恢复通知被触发风暴挤掉）；或对 resolved 使用更松的限额（实现时在配置里用 `resolve_max_per_window`，默认与 firing 相同或略大）。

**配置（建议写入 `eventide.toml`，全局默认；渠道 options 可覆盖）**

```toml
[storm]
# P0
throttle_enabled = true
min_interval_seconds = 60
max_per_window = 20
window_seconds = 60
# throttle_key: "fingerprint" | "labels:alertname,ip" 等
throttle_key = "fingerprint"
```

**实现要点**

| 项 | 说明 |
|----|------|
| 状态存放 | 进程内 `DashMap`/`Mutex<HashMap>` 即可（单机）；key → `{ last_sent_at, window_start, count }`。重启清空可接受（P0）。 |
| 插入点 | `persist_and_notify` 里 `should_notify == true` 之后、`notifier.send` 之前：`if !storm.allow(channel, key, transition, now) { log skipped; continue; }` |
| 单测 | `eventide-server` 或抽 `eventide-core/storm.rs`：同一 key 第 21 次在 60s 内应拒绝；间隔短于 `min_interval` 应拒绝；窗口滚过后应放行。 |
| 控制台 | P0 可不做 UI，靠 toml；有余力在「通知渠道」高级选项暴露覆盖项。 |

**验收**

- [x] 脚本/单测：窗口限额 / 最小间隔 / 滚窗放行（`eventide-core` `storm` 单测）  
- [x] 被节流时 `notify_logs.error=throttled`，告警仍 upsert  
- [x] `eventide.toml` `[storm]` + 启动日志 `storm throttle enabled`  

#### P0 状态：已落地（2026-07）

#### P1 — 时间窗聚合通知（减少「要发的条数」）

**行为**

- 在节流之前（或替代「每条都尝试发」）：按 `group_by` 标签（默认 `alertname`，可配 `alertname,severity` 等）把窗口内多条 BecameFiring **合成一条通知**。  
- 窗口：`aggregate_window_seconds`（如 30）。首条可立即发「开始聚合」或等窗口结束发摘要（**推荐：首条立即发 + 窗口结束补一条 summary**，配置 `aggregate_mode = "head+summary" | "summary_only"`）。  
- 摘要内容至少含：`group_key`、`count`、样例 `ip`/instance 列表（上限 N 个）、时间范围。改 `eventide-notify` 的 `build_text` 或新增 `build_aggregate_text`。  
- 事件库：每条告警仍按 fingerprint upsert；聚合只影响 **通知**，不合并 DB 行（避免破坏恢复对齐）。

**配置**

```toml
[storm]
aggregate_enabled = false
aggregate_window_seconds = 30
group_by = "alertname"
aggregate_mode = "head+summary"
aggregate_sample_labels = "ip,instance,alertIp"
aggregate_sample_limit = 10
```

**实现要点**

| 项 | 说明 |
|----|------|
| 缓冲 | `persist_and_notify` 对 BecameFiring：写入聚合桶 `(channel_id, group_key)`；后台 `tokio` 任务或调度 tick 到期 flush summary。 |
| 与 P0 关系 | 聚合产出的「摘要通知」也走节流（摘要用 `group_key` 作 throttle_key）。 |
| 单测 | 30s 内同 alertname 10 个不同 fingerprint：summary 的 count=10；DB 仍 10 行。 |

**验收**

- [x] 单测：head+summary / summary_only / 仅 1 条跳过摘要（`storm` aggregate 测试）  
- [x] 聚合只影响通知；告警仍按 fingerprint upsert；`notify_logs` 记 `aggregated` / 摘要  

#### P1 状态：已落地（2026-07）

#### P2 — 接入侧削峰 / 背压（可选，偏运维）

**行为**

- HTTP Ingress：过载时返回 `429` 或快速入队（内存有界队列 / 外置 Kafka），避免请求线程堵死。  
- 已有 Kafka Ingress：调小并发、增大消费间隔；积压超阈值时打日志 + metrics（后续），可选「只落库不通知」降级开关 `storm.drop_notify_when_backlog = true`（配合队列深度；无队列时可用「每秒 should_notify 次数」作代理指标）。

**配置**

```toml
[storm]
ingress_max_inflight = 100
degrade_skip_notify = false
```

**验收**

- [x] HTTP 超 `ingress_max_inflight` 返回 **429** `ingress overloaded`  
- [x] `degrade_skip_notify=true` 且高压时 `notify_logs.error=degraded`，告警仍 upsert  
- [x] 单测：inflight 容量拒绝、notify rate 触发 degrade  

#### P2 状态：已落地（2026-07）

#### 明确不做（本阶段）

- 根因抑制树（host down 压掉上层探测）—— 单独立项。  
- ClickHouse / 海量历史仓库 —— 与风暴正交，见架构讨论，不阻塞 P0/P1。  
- ~~多实例共享节流状态（Redis）~~ —— **已随 MySQL + Redis 落地**。

#### 推荐开工顺序（给下次直接开干）

1. ~~新建 `crates/eventide-core/src/storm.rs`（ThrottleGate + 单测）~~ **已完成**  
2. ~~`eventide.toml` / 配置结构加 `[storm]`，`main` 注入 `AppState`~~ **已完成**  
3. ~~改 `notify_pipeline.rs` 接上 ThrottleGate~~ **已完成（P0）**  
4. ~~再做 AggregateBuffer + notify 文案（P1）~~ **已完成**  
5. ~~Ingress 429 / degrade（P2）~~ **已完成**  

抗告警风暴 P0–P2 已齐；更后见 §13.2。

### 13.2 更后可演进

- [x] 企微/飞书以外渠道与更丰富的通知模板（Slack / Telegram；markdown / @）  
- [x] 通知渠道可配置正文模板（`template_firing` / `template_resolved`）  
- [ ] 值班、升级、认领；根因抑制  
- [x] SQLite → MySQL；多实例 + Redis（含共享风暴状态）  
- [x] 多实例调度选主（避免重复评估）  
- [x] 告警历史可选写入 Elasticsearch；列表可切换 MySQL / ES 检索（ClickHouse 仍待定）  
- [ ] 告警历史仓库补强（Kafka 缓冲 → ClickHouse 分析；与热路径进一步分离）  
- [ ] 更多日志后端（ES 等）  
- [ ] Ingress 更多平台适配器（开箱预设）  
- [x] **SNMP Trap 接入骨架**（独立 Trap 服务 → Kafka → Kafka Ingress；统一门户试推送）—— 详见 [§13.3](#133-snmp-trap-接入)  
- [x] SNMP Trap：SNMPv3 USM 接收（TOML `[[snmpv3_users]]`；MD5/SHA/SHA256 + DES/AES128）
- [x] SNMP Trap：多机同时收（UDP LB 推荐）+ VIP 主备备选；Redis 心跳 / 控制台集群视图
- [x] SNMP Trap：策略 MySQL + MIB RustFS + 多实例热加载
- [ ] 控制台前端体验打磨 + ES modules 模块化（见 [§14.2](#142-控制台前端演进)）；有标杆客户/多人协作信号后再考虑 Vite + Vue/React

### 13.3 SNMP Trap 接入

面向大量**主机硬件与网络设备**：设备向 **UDP**（生产 162 / 开发可用 1162）发 Trap，经独立服务解析后写入 Kafka，再由 Eventide **已有 Kafka Ingress** 消费，汇入同一套丰富 / 静默 / 通知流水线。

#### 目标形态

```
主机 / 交换机 / 存储 / BMC …
        │  SNMP Trap (UDP)
        ▼
┌─────────────────────────────┐
│  eventide-trap（独立进程）    │
│  · 收 Trap / 解析 v1·v2c·v3  │
│  · 策略匹配（Redis/MySQL）   │
│  · OID→字段归一化 · 写 Kafka │
└──────────────┬──────────────┘
               │  告警 JSON（与 Ingress 对齐）
               ▼
         Kafka Topic（默认 eventide.snmptrap）
               │
               ▼
┌─────────────────────────────┐
│  Eventide（现有）             │
│  · Kafka Ingress（consumer group）│
│  · 台账丰富 / 静默 / 通知     │
│  · 控制台「SNMP Trap」门户    │
└─────────────────────────────┘
```

**原则：服务可拆，门户合一。** 浏览器只面对 Eventide。  
- **MIB / Trap 策略 CRUD**（上传、OID 树、xlsx 导入导出）在 **Eventide**（`/api/mibs`、`/api/policies`），需配置 `[trap]` 的 S3。  
- Trap **运行态**（health / stats / recent / simulate）经同域 `/trap-api/*` 反代（`[trap] api_url` + 可选 `api_token`）。

#### 当前已实现（骨架）

| 能力 | 状态 |
|------|------|
| `eventide-trap`：UDP v1/v2c/v3(USM) 解析 → Generic Ingress JSON → Kafka | ✅ |
| Trap HTTP：`/api/health`（探针开放）+ stats/recent/simulate/reload（`api_token` Bearer） | ✅ |
| Eventide：`/api/mibs` `/api/policies` CRUD + OID 浏览；`/trap-api` 反代运行态 | ✅ |
| MIB 正文 RustFS + 元数据/策略 MySQL；策略经 Redis 快照分发；Trap 多实例热加载 | ✅ |
| SNMPv3 USM（`[[snmpv3_users]]`） | ✅ |
| 多机 Trap 同时收（UDP LB）+ VIP 主备备选；Redis 心跳 / 控制台集群 | ✅ |

#### 本地联调

```bash
# 1. Eventide（MIB/策略 CRUD 在此）—— MySQL + Redis；MIB 需 [trap] s3_*
cargo run -p eventide-server -- eventide.toml

# 2. Trap 服务（另开终端）—— 收 UDP + 热加载策略/MIB
cargo run -p eventide-trap -- eventide-trap.toml

# eventide.toml [trap]
# api_url = "http://127.0.0.1:8081"
# api_token = "…"   # 与 eventide-trap.toml api_token 一致；反代注入 Bearer
# s3_endpoint / s3_access_key / s3_secret_key / s3_bucket = RustFS
# mib_cache_dir = "data/mib-cache"

# eventide-trap.toml：mysql_url 与 Eventide 同库；同样配置 S3 供热加载 MIB
# redis_url 与 Eventide 同 Redis（策略快照 + pub/sub；失败回退 MySQL）
# [[snmpv3_users]] 配置 USM 用户后可收 SNMPv3 Trap（authNoPriv / authPriv）
# kafka_partitions 须与 Topic 分区数、Kafka Ingress options.partitions 一致（按 peer IP 散列）
# policy_reload_secs 兜底轮询（Redis 优先，感知 Server 侧 CRUD）

# 3. 控制台 → MIB 库 / Trap 策略（走 /api/...）
# 4. 接入配置 → SNMP Trap →「试推送」（走 /trap-api/...）
# 5. 「告警接入」新建 Kafka Ingress：brokers + options.topic=eventide.snmptrap + partitions=8
# 6. 扩分区示例：kafka-topics --alter --topic eventide.snmptrap --partitions 8
```

无 Kafka 时可将 `eventide-trap.toml` 的 `kafka_brokers` 留空，用试推送勾选「仅预览 JSON」验证归一化字段。

#### 策略分发（MySQL → Redis → Trap）

1. **MySQL `trap_policies`**：唯一真相源；控制台 CRUD 写库。
2. **Eventide** 启动及每次策略变更后：写入 Redis 键 `eventide:trap:policies:snapshot`，并向 `eventide:trap:policies:changed` **PUBLISH**。
3. **Trap**：启动优先读 Redis 快照；订阅 channel 近实时重载；`policy_reload_secs` 轮询兜底；Redis 不可用时回退 MySQL。

#### 多机 Trap 同时收（HA）

**推荐**：前面挂 **UDP 负载均衡**（云 NLB / Nginx stream / LVS），三台及以上 Trap **并行收包** → 同一 Kafka Topic。  
**备选**：无 UDP LB 时用 Keepalived VIP（同时只有 MASTER 收包）。  

- 部署说明与示例：[`deploy/trap-ha/`](deploy/trap-ha/)（含 `nginx-udp.conf.example`、Keepalived）
- Trap 配置：每机不同 `instance_id`；`ha_vip` 可填 LB 入口；`heartbeat_secs` 开心跳
- 控制台 **SNMP Trap → 集群心跳**（`GET /api/trap/instances`）

#### 职责划分

| 能力 | 归属 | 说明 |
|------|------|------|
| MIB / 策略 **CRUD**、OID 树、xlsx 导入导出 | **Eventide** | JWT + IAM；写 MySQL + RustFS；策略同步 Redis |
| MIB 正文（对象存储）+ 元数据（MySQL） | 共享存储 | Server 写；Trap 热加载读 |
| Trap 策略匹配（内存） | **Trap 服务** | Redis 快照 + pub/sub；失败回退 MySQL |
| 监听 UDP、解析、指纹、写入 Kafka | **Trap 服务** | UDP 扇入；多机可并行写同一 Topic（按 peer IP 散列分区） |
| Kafka Ingress 消费、入库、通知、静默 | **Eventide** | consumer group 多实例扩容；复用现有能力 |
| IP→机房/负责人等**运营台账丰富** | **Eventide** | 现有「告警丰富 / 台账」 |
| Trap **协议侧**丰富（OID→告警名、severity、`${var}` 摘要） | **Trap 服务**（写 Kafka 前完成） | 与台账丰富分层 |

#### Kafka 告警约定

Trap 服务写出单条告警对象（可被 Generic / 自动识别），至少包含：

- `status` / `severity` / `fingerprint`
- `labels`：`alertname`、`ip`、`trap_oid`、`source=ingress:snmptrap` 等
- `annotations.summary`

#### 推荐实现顺序

1. [x] Trap 服务骨架：收 Trap（v1/v2c）→ 固定字段 JSON → 写 Kafka；门户试推送  
2. [x] 统一门户：Eventide 侧栏入口 + `/trap-api` 反代（JWT）；Trap HTTP `api_token`（系统设置可轮换，Redis 热更新）  
3. [x] MIB / 策略 CRUD 在 Eventide（`/api/mibs`、`/api/policies`）；Trap 热加载；OID 树与 xlsx 导出  
4. [x] Trap 策略 CRUD、摘要 `${变量}` 生效、Excel(xlsx) 导入导出  
5. [x] SNMPv3 USM 接收（`eventide-trap.toml` → `[[snmpv3_users]]`）  
6. [x] 多机 Trap 同时收（UDP LB）+ VIP 主备备选；实例心跳  
7. [x] 与台账丰富联调（`labels.ip` + 台账 `key_label=ip`；seed 含 `trap_hosts`）  
8. [x] （可选）MIB 浏览器 SNMP Get；指标/积压监控（Trap 页 stats 卡片 + Kafka lag）   

> MIB 节点详情可对目标设备做 **SNMPv2c Get**（`POST /api/snmp/get`）。Trap 门户展示接收/解析/Kafka 计数，并查询 SNMP Trap Ingress 消费组积压。

#### Trap × 台账联调（验收）

1. 跑 seed（会种台账 `trap_hosts` 与规则「Trap 设备台账丰富」）：
   ```powershell
   powershell -File scripts/seed-demo.ps1
   ```
2. 控制台 **台账数据**：确认 `trap_hosts`，匹配键 `ip`，含 `10.0.0.88` / `10.0.0.99`。  
3. **告警丰富**：规则匹配 `source=ingress:snmptrap`，引用该台账，`write_labels=true`。  
4. Trap 试推送或真实 Trap，peer IP 用台账中的 IP。  
5. **告警事件**：描述应含 `主机名 / 机房 / 联系人`；标签可见 `trap_hosts.*`。  

要点：Trap 写的是 `labels.ip`（不是 `instance`）；台账匹配键必须是 `ip`。

- Eventide 主进程内嵌监听 **162**
- 在 Eventide 内维护完整厂商 MIB 与 OID 策略编辑
- 用 Eventide「告警丰富」替代 Trap 策略摘要

---
## 14. 开发与构建

```bash
# 全 workspace 测试
cargo test --workspace

# 仅编译服务
cargo build -p eventide-server --release

# Trap 服务
cargo build -p eventide-trap --release

# 运行
./target/release/eventide eventide.toml
./target/release/eventide-trap eventide-trap.toml
```

### 14.1 CI 多平台打包

GitHub Actions：[`.github/workflows/build.yml`](.github/workflows/build.yml)  
麒麟任务参考 [axleops `build-linux.yml`](https://github.com/lvpenglive/axleops/blob/main/.github/workflows/build-linux.yml)：在社区镜像 `hxsoong/kylin:v10-sp3` 内编译。

| Artifact | 说明 |
|----------|------|
| `eventide-linux-x86_64` | Ubuntu runner，glibc 动态链接 |
| `eventide-kylin-v10-sp3-x86_64` | 银河麒麟 V10 SP3 容器内编译（贴近真机 glibc） |
| `eventide-windows-x86_64` | Windows MSVC |

每个包含：`eventide` / `eventide-trap` / `eventide-license`、控制台 `static/`、示例配置 `*.toml.example`。

- **push / PR / 手动**：上传 Actions Artifacts（保留 30 天）
- **打 tag（`v*`）**：汇总进 GitHub Release，例如 `git tag v0.1.0 && git push origin v0.1.0`

本地也可对照示例配置：

```bash
cp eventide.toml.example eventide.toml
cp eventide-trap.toml.example eventide-trap.toml
```

### 14.2 控制台前端演进

**原则**：继续「单二进制 + 无 Node 构建」交付；先把关键路径做成像样的产品壳，再谈换栈。不要为了「看起来现代」提前上 monorepo / 微前端。

#### 现状

| 项 | 说明 |
|----|------|
| 形态 | `static/` 内嵌 SPA；`ServeDir` 直出 |
| 入口 | `index.html` → `type="module"` 加载 `assets/app.js` |
| 已拆模块 | `assets/js/api.js`（鉴权请求）、`ui.js`（toast / modal / esc 等）、`pages/alerts.js`（告警列表与详情） |
| 其余页面 | 仍集中在 `app.js`（路由 + Trap / 规则 / 丰富 / IAM 等） |

内嵌静态页**够用**（配置、查看、操作都能完成）；对标商业控制台时，短板主要在**体验一致性**与**长期扩展成本**（大文件难协作），而不是「技术过时」。

#### 阶段 0 — 体验打磨（不换栈）

1. **统一交互范式**：列表页统一为「筛选 + 表格/卡片 + 分页 + 弹窗编辑」；按钮文案、空状态、错误提示一套规范。  
2. **关键路径优先**：登录 → 告警列表 → 详情 → 静默；Trap 策略；许可导入。这三条做到顺滑，比换框架更像商业产品。  
3. **性能**：告警大列表坚持服务端分页；减少整页无意义重绘；接口失败有明确提示。  
4. **视觉收敛**：保持现有品牌字体与 dusk 风格，克制堆卡片/特效。

#### 阶段 1 — ES modules 模块化（进行中）

目标结构（无构建工具，浏览器原生 `import`）：

```
static/
  index.html
  assets/
    app.css
    app.js              # 路由 + 组装
    js/
      api.js            # fetch / JWT / 402 license_readonly
      ui.js             # toast、modal、confirm、表格小工具
      pages/
        alerts.js       # ✅ 已拆
        trap.js         # 待拆
        enrich.js       # 待拆
        settings.js     # 待拆（含许可）
        …
```

优先级：告警相关 ✅ → Trap / MIB / 策略 → 丰富与台账 → 系统设置（许可）→ 其余 CRUD 页。  
可选辅助脚本：`scripts/split-console-modules.mjs`（对照拆分，非常规构建步骤）。

#### 阶段 2 — 有信号再换栈（可选）

出现以下信号再考虑 Vue 3 / React + Vite：

- 要做复杂仪表盘、可配置工作台、大量表单校验与草稿  
- 2 人以上长期改前端  
- 客户明确对比竞品 UI，且成交卡在「看起来不专业」

推荐仍贴合当前部署：

- 单独 `console/` 工程，`npm run build` → 产物写入 `static/`  
- 服务端继续 `ServeDir`，**不**拆成第二个部署包  

#### 性价比排序

| 顺序 | 做什么 | 为什么 |
|------|--------|--------|
| 1 | 告警列表 / 详情体验 | 用户每天都看 |
| 2 | 继续拆 `app.js`（Trap、丰富、设置） | 后续改动的基础 |
| 3 | 统一表格 / 表单 / Toast 约定 | 商业感来自一致性 |
| 4 | （可选）Vite + Vue/React | 有客户与人力再上 |

#### 明确不做（现阶段）

- 全量重写成 Ant Design / 同类中后台模板  
- 为换栈停功能开发  
- 前后端分离成两个独立部署单元（牺牲「单二进制」卖点）

### 14.3 本地调试日志

```bash
# Windows PowerShell
$env:RUST_LOG="info,eventide=debug"
cargo run -p eventide-server -- eventide.toml
```

核心单测集中在 `eventide-core`（比较符、`for` 状态机、告警标识、Ingress 解析与字段映射、模板截取、告警丰富）。

### 14.4 控制台 E2E

依赖本机已启动的 Eventide（默认 `http://127.0.0.1:8080`）与 Playwright：

```powershell
pip install playwright
python -m playwright install chromium

$env:EVENTIDE_USER="admin"
$env:EVENTIDE_PASS="admin123"
python scripts/e2e_console.py
```

覆盖：错误/正确登录、URL 不泄露密码、侧栏各页加载、接入帮助与一键创建、试推送弹窗、告警/设置页、退出登录。失败时截图写入 `docs/e2e-artifacts/`（可用 `EVENTIDE_E2E_ARTIFACTS=0` 关闭）。

相关脚本：`scripts/capture_console_screenshots.py`（手册配图采集）。

---

## 附录：术语表

| 术语 | 含义 |
|------|------|
| PromQL | Prometheus 查询语言 |
| LogQL | Loki 日志查询语言 |
| fingerprint | 告警标识（相同标签合并为同一条事件） |
| for | 条件需持续满足的时间 |
| firing / resolved | 告警触发 / 恢复 |
| Ingress | 外部告警接入入口 |
| Datasource | 可查询的原始数据源 |
| Enrich | 告警丰富（台账补字段 / 模板改写） |
| Lookup / 台账 | 键值对照表，供丰富规则查表 |
| field mapping | 接入侧把任意 JSON 映射为统一告警字段 |
| storm / 告警风暴 | 短时间大量相似告警涌入；靠节流、聚合、削峰消化 |
| SNMP Trap | 设备主动推送的 SNMP 告警（常 UDP/162）；由独立 `eventide-trap` 解析后进 Kafka |
| MIB | SNMP 管理信息库；描述 OID 含义，供 Trap 策略与解析使用 |
| Trap 策略 | OID→级别/摘要/关键字等规则，决定 Trap 如何归一成告警 |

---

如需二次开发，建议从 `eventide-core` 的模型与 `apply_evaluation` / `apply_ingress` / `enrich_alert` 读起，再看 `eventide-server` 的 `scheduler` 与 `notify_pipeline`。
