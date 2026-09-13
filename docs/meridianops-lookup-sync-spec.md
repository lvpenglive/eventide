# Eventide 外表（Lookup）同步对接说明

> 面向：**MeridianOps** 工程 / AI Agent  
> 目标：从 MeridianOps（CMDB 主库）向 Eventide 同步**部分**台账外表数据，供告警丰富（enrich）使用。  
> 读完本文即可直接实现同步 Job，无需再问 Eventide 侧业务细节。

---

## 1. 背景与职责边界

| 系统 | 职责 |
|------|------|
| **MeridianOps** | 主机资产、联系人、业务归属等 **主数据**；定时/变更后把投影推到 Eventide |
| **Eventide** | 告警归一、去重、静默、风暴、通知；外表仅作 **只读投影缓存**，用于 enrich |

**不要：**

- 在 Eventide 回写主机/联系人主数据
- 把所有外表都同步（只同步配置清单内的表）
- 用 Eventide 管理员账号密码长期登录做同步（用专用 sync token）

告警反向通道（Eventide → MeridianOps webhook）已存在，**与本任务无关**。本任务只做：**CMDB → Eventide Lookup rows**。

---

## 2. 环境与鉴权

### 2.1 Base URL

```text
EVENTIDE_BASE_URL=http://<eventide-host>:8080
```

本地联调示例：`http://127.0.0.1:8080`

### 2.2 Sync Token

向 Eventide 管理员索取 **外表同步 Token**（控制台路径：v2 → 系统设置 →「b2) 外表同步 Token（MeridianOps）」→ 重新生成）。

也可由对方在 `eventide.toml` 配置：

```toml
[lookup_sync]
token = "lks_xxxxxxxx..."
# 可选：只允许这些 lookup UUID；空 = 仅允许已勾选 external_sync 的外表
# allowlist = ["579dc0d5-7e88-49bc-8314-7a0a8583996a"]
```

请求头：

```http
Authorization: Bearer <LOOKUP_SYNC_TOKEN>
Content-Type: application/json
```

Token 轮换后旧 token 立即失效，需更新 MeridianOps 配置。

### 2.3 允许写入哪些表

满足其一即可：

1. **allowlist 非空**：`lookup_id` 必须在 allowlist 中  
2. **allowlist 空**：目标外表必须在 Eventide 控制台勾选了 **外部同步**（`external_sync=true`）

未满足 → HTTP **403**。

---

## 3. API 契约（核心）

### 3.1 同步接口（唯一必需）

```http
PUT {EVENTIDE_BASE_URL}/api/lookups/{lookup_id}/rows
Authorization: Bearer <LOOKUP_SYNC_TOKEN>
Content-Type: application/json
```

**Path**

| 参数 | 说明 |
|------|------|
| `lookup_id` | Eventide 外表 UUID（向 Eventide 管理员要，或 GET 列表后配置进清单） |

**Body**

```json
{
  "rows": {
    "10.20.0.1": {
      "主机名": "web01",
      "机房": "DC-A",
      "联系人": "张三",
      "电话": "13800000000"
    },
    "10.20.0.2": {
      "主机名": "web02",
      "机房": "DC-B",
      "联系人": "李四",
      "电话": "13900000000"
    }
  },
  "reject_empty": true,
  "sync_source": "MeridianOps"
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `rows` | `object` | 是 | key → 属性 map；**整表覆盖**（不是增量 patch） |
| `rows` 的 key | `string` | — | 必须与 Eventide 该表的 `key_label` 语义一致，通常是 **IP** |
| `rows[key][col]` | `string` | — | 列名与 Eventide enrich 模板约定一致（见 §4） |
| `reject_empty` | `bool` | 否 | **默认 `true`**：`rows` 为空时拒绝覆盖，防误清空 |
| `sync_source` | `string` | 否 | 建议固定 `"MeridianOps"`，写入外表元数据 |

**成功响应**：`200`，返回完整 Lookup 对象，含：

- `id`, `name`, `key_label`, `rows`
- `external_sync`（同步成功后为 `true`）
- `synced_at`（ISO8601 UTC）
- `sync_source`

**错误**

| HTTP | 含义 | 处理建议 |
|------|------|----------|
| 400 | 空 rows 且 `reject_empty=true`；或 body 非法 | 不要用空结果覆盖；检查 JSON |
| 401 | token 缺失/错误/过期 | 检查 Bearer；联系对方轮换 token |
| 403 | 不在 allowlist / 未开 external_sync | 检查清单与 Eventide 勾选 |
| 404 | `lookup_id` 不存在 | 修正配置中的 UUID |
| 402 | 许可证只读（少见） | 联系 Eventide 管理员 |
| 5xx | 服务异常 | 重试 + 告警 |

### 3.2 辅助：列出外表（可用用户 JWT，非 sync token）

若需要自动发现 id（一般用配置清单即可）：

```http
POST /api/auth/login
{"username":"...","password":"..."}

GET /api/lookups
Authorization: Bearer <user_jwt>
```

同步 Job **生产环境不要依赖用户密码**；只在初始化时人工配置 `lookup_id`。

### 3.3 不要用这些接口做日常同步

| 接口 | 原因 |
|------|------|
| `PUT /api/lookups/{id}` | 整表 CRUD，需 JWT + 必须带非空 rows/name；易误改元数据 |
| 告警 ingress / webhook | 那是告警通道，不是 CMDB |

---

## 4. 数据映射约定（请与 Eventide 对齐后写死）

### 4.1 推荐第一张表：`hosts`

| 项 | 约定值（示例，以联调确认为准） |
|----|-------------------------------|
| Eventide 外表名 | `hosts` 或 `hosts_meridian_verify` |
| `lookup_id` | **向 Eventide 要 UUID，写入 MeridianOps 配置** |
| `key_label` | `ip` |
| rows key | 主机管理 IP（与告警 `labels.ip` / `labels.instance` 能对上） |

**建议列（列名建议直接用下面中文，与 Eventide enrich 模板一致）：**

| 列名 | 来源（MeridianOps） | 说明 |
|------|---------------------|------|
| `主机名` | hostname / CI 名 | 必填优先 |
| `机房` | IDC / 机房字段 | 可选 |
| `联系人` | owner / 值班人姓名 | 建议 |
| `电话` | mobile | 可选 |
| `邮箱` | email | 可选 |
| `业务线` | biz / app | 可选 |

> 列名一旦约定，**禁止单边改名**。Eventide enrich 用的是 `{{labels.外表名.列名}}`，改列名会静默丰富失败。

### 4.2 转换伪代码

```text
rows = {}
for each host in MeridianOps.query_hosts(for_sync=true):
  ip = normalize_ipv4(host.mgmt_ip)   # 去空格；不要带端口
  if ip empty: skip
  rows[ip] = {
    "主机名": host.name or "",
    "机房": host.datacenter or "",
    "联系人": host.owner_name or "",
    "电话": host.owner_phone or "",
    "邮箱": host.owner_email or "",
    "业务线": host.biz_line or ""
  }
# 然后 PUT 整表 rows
```

### 4.3 多张表

支持「只同步部分外表」：配置数组，每项一个 Job 目标：

```json
{
  "targets": [
    {
      "lookup_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
      "name": "hosts",
      "key_field": "mgmt_ip",
      "columns": {
        "主机名": "name",
        "机房": "datacenter",
        "联系人": "owner_name",
        "电话": "owner_phone"
      }
    }
  ]
}
```

未出现在 `targets` 的 Eventide 外表 **不要碰**。

---

## 5. 同步 Job 行为要求

### 5.1 触发

1. **定时全量**：默认每 5 分钟（可配置）  
2. **变更触发**：主机/联系人增删改后 debounce（如 30～60s）再跑一轮全量  

第一期用 **全量覆盖** 即可（与 API 语义一致）。行数很大（例如 >5 万）再评估增量。

### 5.2 安全规则（必须）

1. `reject_empty: true`  
2. 若查询结果 `rows` 为空 → **跳过 PUT**，打 ERROR 日志并告警（防止把 Eventide 表清空）  
3. 单表失败不影响其他 `targets`（继续推下一张）  
4. HTTP 失败：指数退避重试 3 次（如 1s/2s/4s），仍失败则告警  

### 5.3 日志字段（建议）

每次同步记录：

- `lookup_id`, `row_count`, `http_status`, `latency_ms`, `synced_at`（若响应有）  
- 错误 body 文本  

### 5.4 幂等

同一份 `rows` 重复 PUT 是安全的（覆盖写）。无需版本号；以 Eventide `synced_at` 作观测即可。

---

## 6. 联调步骤（给实施同学）

1. Eventide：建外表（或指定已有表）→ 勾选 **外部同步** → 记下 `lookup_id`  
2. Eventide：系统设置生成 **外表同步 Token** → 交给 MeridianOps  
3. MeridianOps：配置 `EVENTIDE_BASE_URL`、`LOOKUP_SYNC_TOKEN`、`targets[]`  
4. 手动跑一次 Job，检查 Eventide `GET /api/lookups/{id}`：`rows`、`synced_at`、`sync_source=MeridianOps`  
5. 在 MeridianOps 改一台主机名 → 再同步 → Eventide 对应 IP 行更新  
6.（可选）Eventide 推一条带该 `ip` 的告警，确认 enrich 带出 `主机名`/`联系人`  

### 6.1 curl 示例

```bash
TOKEN='lks_your_token_here'
BASE='http://127.0.0.1:8080'
LOOKUP_ID='579dc0d5-7e88-49bc-8314-7a0a8583996a'

curl -sS -X PUT "$BASE/api/lookups/$LOOKUP_ID/rows" \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "reject_empty": true,
    "sync_source": "MeridianOps",
    "rows": {
      "10.20.0.11": {
        "主机名": "web-verify-01",
        "机房": "DC-A",
        "联系人": "张三"
      }
    }
  }'
```

期望：HTTP 200，JSON 中 `synced_at` 有值，`rows` 含上述 IP。

### 6.2 负面用例

```bash
# 空表应 400
curl -sS -o /dev/null -w "%{http_code}\n" -X PUT "$BASE/api/lookups/$LOOKUP_ID/rows" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"rows":{},"reject_empty":true}'

# 未授权外表应 403
curl -sS -o /dev/null -w "%{http_code}\n" -X PUT "$BASE/api/lookups/<non-external-id>/rows" \
  -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"rows":{"1.1.1.1":{"x":"y"}},"sync_source":"MeridianOps"}'
```

---

## 7. 交付清单（MeridianOps 侧 Done 标准）

- [ ] 配置项：`EVENTIDE_BASE_URL`、`LOOKUP_SYNC_TOKEN`、`targets`（含 lookup_id + 列映射）  
- [ ] 同步服务/脚本：定时 + 变更 debounce  
- [ ] 空结果跳过 PUT + 失败重试 + 失败告警  
- [ ] 至少 1 张 hosts 表联调通过（§6）  
- [ ] README/运维说明：如何轮换 token、如何加一张新外表到清单  
- [ ] **不**在代码里写死 Eventide admin 密码  

---

## 8. 给实现 AI 的直接任务说明（可复制）

请在 MeridianOps 仓库实现「Eventide Lookup 同步」模块，要求如下：

1. 新增配置结构：`eventide.base_url`、`eventide.lookup_sync_token`、`eventide.lookup_targets[]`（每项含 `lookup_id`、可选 `name`、字段映射）。  
2. 实现 `SyncEventideLookups`：按 targets 从本系统 CMDB 拉数 → 组装 `rows` → `PUT /api/lookups/{id}/rows`，Header 使用 Bearer sync token；body 含 `reject_empty=true`、`sync_source=MeridianOps`。  
3. `rows` 为空时禁止调用 PUT，并打错误日志/告警。  
4. 注册定时任务（默认 5min）与资产变更后的 debounce 触发。  
5. 对 401/403/404/5xx 分类日志；5xx 重试 3 次。  
6. 提供本地联调文档与一次成功的 curl/集成测试样例。  
7. 不要实现告警接收逻辑；不要写 Eventide 用户名密码登录做生产同步。  

列映射默认按本文 §4.1；若本系统字段名不同，用配置映射，不要改 Eventide 列名。

---

## 9. 联系 Eventide 侧需提供的信息

向 Eventide 管理员索取/确认：

1. `EVENTIDE_BASE_URL`  
2. `LOOKUP_SYNC_TOKEN`（及后续轮换方式）  
3. 每张要同步外表的：`lookup_id`、`name`、`key_label`、列名列表  
4. 是否配置了 allowlist（若有，把你们的 id 加进去）  
5. 网络：MeridianOps → Eventide:8080 是否互通  

---

## 10. 版本备注

- 接口：`PUT /api/lookups/{id}/rows`（Eventide 已实现）  
- 设置：`GET|PUT /api/settings/lookup-sync`（Eventide 控制台用；MeridianOps Job 不需要调）  
- 文档日期：2026-09-13  
