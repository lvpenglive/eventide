# Trap 多机同时收包（推荐）+ VIP 主备（备选）

`eventide-trap` **天然支持多实例并行收包** → 写同一 Kafka Topic。  
设备只填一个目标地址时，前面加 **UDP 负载均衡**，三台（或更多）**同时分担流量**。

控制台 **SNMP Trap → 集群心跳** 可看到各实例是否存活（Redis heartbeat）。

---

## 推荐：UDP LB + 多台 Trap 同时收

```
设备 → LB入口IP:162（唯一 Trap 目标）
              │
         UDP 负载均衡
      （云 NLB / LVS / Nginx stream …）
         ┌────┼────┐
      Trap-1 Trap-2 Trap-3     ← 三台都在收
         └────┼────┘
           Kafka Topic
              │
         Eventide Ingress（consumer group）
```

每个 UDP 包只进 **一台** Trap，三台一起扛量，一般不会因分流产生重复告警。

### Trap 侧配置（每台一份 toml）

| 项 | 要求 |
|----|------|
| `instance_id` | **每台不同**（如 `trap-1` / `trap-2` / `trap-3`） |
| `mysql_url` / `redis_url` / Kafka / S3 | **相同** |
| `listen_udp` | `0.0.0.0:162`（或本机 IP；需权限） |
| `ha_vip` | 可填 **LB 入口 IP**（仅展示）；留空也行 |
| `heartbeat_secs` | `> 0`（默认 5），便于控制台看集群 |

示例见仓库根目录 `eventide-trap.example.toml`。

### UDP LB 示例

- 云厂商：**UDP 网络负载均衡 / NLB**，后端挂三台 Trap 的 `:162`，健康检查可用 TCP/HTTP（HTTP 探 `http://host:8081/api/health`）。
- 自建 Nginx（需 `stream` + UDP）：见 [`nginx-udp.conf.example`](nginx-udp.conf.example)。
- LVS / HAProxy（UDP）等同理。

### 验证

1. 三台 Trap 启动后，控制台应看到 3 个 `instance_id` 心跳。
2. 向 **LB 入口** 发 Trap，任一台（或轮询）计数增加；Kafka Ingress 正常入库。
3. 停掉其中一台：其余继续收，LB 摘掉不健康节点。

### 不推荐：设备填 3 个 Trap IP

会把**同一条** Trap 复制到三台 → Kafka 可能进 3 份，只能靠 fingerprint 去重，浪费且易抖。优先用 UDP LB。

---

## 备选：Keepalived VIP 主备（同时只有一台收）

没有 UDP LB、又必须单 IP 时，可用 VRRP 漂移 VIP：**同一时刻仅 MASTER 收包**，备机热备。  
**不能**用这种方式实现「三台同时收」。

- [`keepalived.conf.example`](keepalived.conf.example)
- [`check_trap.sh`](check_trap.sh) — 探活 `/api/health`

```
设备 → VIP:162
         │
    Keepalived（单主）
    ┌────┴────┐
  MASTER    BACKUP…
```

---

## 探活说明

- `/api/health` **始终开放**（给 LB / Keepalived 用）。
- stats / simulate / reload 仍走 `api_token`（控制台「系统设置 → Trap HTTP Token」）。
