/* Eventide console — alerts page + helpers */
export const ALERT_VIEW_KEY = "eventide_alert_view";
export const ALERT_AUTO_KEY = "eventide_alert_auto";
export const ALERT_PAGE_SIZE = 50;

/**
 * Factory so alerts can use app-level navigate / editSilence / modal without cycles.
 */
export function createAlertsModule(deps) {
  const {
    api,
    state,
    toast,
    esc,
    fmtTime,
    severityOptions,
    severityLabel,
    navigate,
    setActions,
    openModal,
    closeModal,
    showCtxMenu,
    copyText,
    editSilence,
    showNotifyLogDetail,
  } = deps;

  let alertsTimer = null;
  function _setTimer(t) {
    alertsTimer = t;
  }
  function _stop() {
    if (alertsTimer) {
      clearInterval(alertsTimer);
      alertsTimer = null;
    }
  }

function getAlertView() {
  const v = localStorage.getItem(ALERT_VIEW_KEY);
  return v === "table" ? "table" : "cards";
}

function setAlertView(v) {
  localStorage.setItem(ALERT_VIEW_KEY, v === "table" ? "table" : "cards");
}

function getAlertAutoRefresh() {
  const v = Number(localStorage.getItem(ALERT_AUTO_KEY));
  return [0, 15, 30, 60].includes(v) ? v : 15;
}

function setAlertAutoRefresh(v) {
  const n = Number(v);
  localStorage.setItem(ALERT_AUTO_KEY, String([0, 15, 30, 60].includes(n) ? n : 15));
}

function scheduleAlertsAutoRefresh(tick) {
  _stop();
  const sec = getAlertAutoRefresh();
  if (!sec || typeof tick !== "function") return;
  _setTimer(setInterval(() => {
    if (state.page !== "alerts") {
      _stop();
      return;
    }
    tick();
  }, sec * 1000));
}

function normalizeAlertList(data) {
  if (Array.isArray(data)) {
    const items = data;
    return {
      items,
      total: items.length,
      page: 1,
      limit: items.length || ALERT_PAGE_SIZE,
      status_counts: {
        firing: items.filter((a) => a.status === "firing").length,
        pending: items.filter((a) => a.status === "pending").length,
        resolved: items.filter((a) => a.status === "resolved").length,
      },
    };
  }
  const items = Array.isArray(data?.items) ? data.items : [];
  const sc = data?.status_counts || {};
  return {
    items,
    total: Number(data?.total) || items.length,
    page: Number(data?.page) || 1,
    limit: Number(data?.limit) || ALERT_PAGE_SIZE,
    status_counts: {
      firing: Number(sc.firing) || 0,
      pending: Number(sc.pending) || 0,
      resolved: Number(sc.resolved) || 0,
    },
  };
}

function alertPagerHtml(page, pages, total, limit) {
  if (total <= 0) return "";
  const from = (page - 1) * limit + 1;
  const to = Math.min(page * limit, total);
  return `<div class="alert-pager">
    <span class="alert-pager-meta">第 ${from}–${to} 条，共 ${total} 条</span>
    <div class="alert-pager-btns">
      <button type="button" class="ghost" data-alert-page="prev" ${page <= 1 ? "disabled" : ""}>上一页</button>
      <span class="alert-pager-page">${page} / ${pages}</span>
      <button type="button" class="ghost" data-alert-page="next" ${page >= pages ? "disabled" : ""}>下一页</button>
    </div>
  </div>`;
}

function bindAlertPager(root, reload) {
  root.querySelectorAll("[data-alert-page]").forEach((btn) => {
    btn.onclick = () => {
      const cur = Number(state.alertFilters?.page) || 1;
      const next = btn.dataset.alertPage === "prev" ? cur - 1 : cur + 1;
      if (next < 1) return;
      state.alertFilters = { ...(state.alertFilters || {}), page: next };
      reload();
    };
  });
}

function alertTargetIp(a) {
  const l = a.labels || {};
  return l.alertIp || l.ip || l.ipaddr || l.instance || l.host || l.hostname || "";
}

function alertCards(rows, ingressMap) {
  return `<div class="alert-feed">${rows
    .map((a, i) => {
      const name = alertDisplayName(a);
      const summary = alertSummary(a);
      const dur = alertDuration(a);
      const src = alertSourceLabel(a, ingressMap);
      const val = fmtValue(a.value);
      const ip = alertTargetIp(a);
      const chips = labelChips(
        a.labels,
        ["alertname", "severity", "source", "alertIp", "ip", "ipaddr", "instance", "host", "hostname"],
        3
      );
      return `<article class="alert-card status-${esc(a.status)} sev-${esc(
        a.severity
      )}" data-alert-idx="${i}" tabindex="0" role="button">
        <div class="ac-rail" aria-hidden="true"></div>
        <div class="ac-body">
          <div class="ac-top">
            <div class="ac-title-row">
              <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
              <span class="sev sev-${esc(a.severity)}">${esc(severityLabel(a.severity))}</span>
              <h3 class="ac-name">${esc(name)}</h3>
            </div>
            <button type="button" class="ghost ac-detail" data-alert-detail="${i}">详情</button>
          </div>
          <p class="ac-summary">${esc(summary || "无描述")}</p>
          <div class="ac-meta">
            <span>${esc(src)}</span>
            ${
              ip
                ? `<span class="dot">·</span><span class="ac-ip" title="告警 IP">IP ${esc(ip)}</span>`
                : ""
            }
            <span class="dot">·</span>
            <span>持续 ${esc(dur)}</span>
            ${val !== "—" ? `<span class="dot">·</span><span class="mono">值 ${esc(val)}</span>` : ""}
            <span class="dot">·</span>
            <span>更新于 ${esc(fmtTime(a.last_evaluated_at))}</span>
          </div>
          <div class="alert-labels">${chips}</div>
        </div>
      </article>`;
    })
    .join("")}</div>`;
}

function silenceFromAlert(a) {
  if (!a) return;
  const matchers = { ...(a.labels || {}) };
  delete matchers.source;
  editSilence({
    comment: `静默 ${alertDisplayName(a)}`,
    matchers,
    rule_id: null,
  });
}

function filterAlertsByQuery(q) {
  const query = String(q || "").trim();
  if (!query) {
    toast("筛选关键词为空", true);
    return;
  }
  state.alertFilters = { ...(state.alertFilters || {}), q: query, page: 1 };
  navigate("alerts");
}

function filterAlertsByIp(ip) {
  const value = String(ip || "").trim();
  if (!value) {
    toast("IP 为空", true);
    return;
  }
  state.alertFilters = { ...(state.alertFilters || {}), ip: value, page: 1 };
  navigate("alerts");
}

function openAlertContextMenu(e, a, ingressMap) {
  if (!a) return;
  e.preventDefault();
  e.stopPropagation();
  const summary = alertSummary(a);
  const ip = alertTargetIp(a);
  const name = alertDisplayName(a);
  const nameOk = !!name && name !== "未命名告警";
  showCtxMenu(e.clientX, e.clientY, [
    {
      label: "查看详情",
      onClick: () => showAlertDetail(a, ingressMap || {}),
    },
    {
      label: "据此静默",
      onClick: () => silenceFromAlert(a),
    },
    { sep: true },
    {
      label: "复制告警描述",
      disabled: !summary,
      onClick: () => copyText(summary, "告警描述"),
    },
    {
      label: "复制告警 IP",
      disabled: !ip,
      onClick: () => copyText(ip, "告警 IP"),
    },
    {
      label: "复制告警标识",
      disabled: !a.fingerprint,
      onClick: () => copyText(a.fingerprint, "告警标识"),
    },
    { sep: true },
    {
      label: "按此告警名称筛选",
      disabled: !nameOk,
      onClick: () => filterAlertsByQuery(name),
    },
    {
      label: "按此 IP 筛选",
      disabled: !ip,
      onClick: () => filterAlertsByIp(ip),
    },
  ]);
}

function bindAlertCards(container, rows, ingressMap) {
  if (!container || !rows?.length) return;
  const open = (i) => showAlertDetail(rows[i], ingressMap || {});
  container.querySelectorAll(".alert-card").forEach((card) => {
    card.addEventListener("click", (e) => {
      if (e.target.closest("[data-alert-detail]")) return;
      open(Number(card.dataset.alertIdx));
    });
    card.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        open(Number(card.dataset.alertIdx));
      }
    });
    card.addEventListener("contextmenu", (e) => {
      const idx = Number(card.dataset.alertIdx);
      openAlertContextMenu(e, rows[idx], ingressMap);
    });
  });
  container.querySelectorAll("[data-alert-detail]").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      open(Number(btn.dataset.alertDetail));
    });
  });
}

function alertTableGrid(rows, ingressMap) {
  return `<table class="data alert-table"><thead><tr>
    <th>状态</th><th>级别</th><th>告警名称</th><th>告警描述</th><th>告警 IP</th><th>当前值</th>
    <th class="col-src">来源</th><th class="col-time">开始时间</th><th class="col-dur">持续</th><th class="col-time">最后更新</th><th class="col-act"></th>
  </tr></thead><tbody>${rows
    .map((a, i) => {
      const name = alertDisplayName(a);
      const summary = alertSummary(a);
      const dur = alertDuration(a);
      const src = alertSourceLabel(a, ingressMap);
      const ip = alertTargetIp(a);
      return `<tr data-alert-idx="${i}">
    <td><span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span></td>
    <td><span class="sev sev-${esc(a.severity)}">${esc(severityLabel(a.severity))}</span></td>
    <td>
      <div class="alert-name">${esc(name)}</div>
      <div class="alert-labels">${labelChips(
        a.labels,
        ["alertname", "severity", "source", "alertIp", "ip", "ipaddr", "instance", "host", "hostname"],
        3
      )}</div>
    </td>
    <td title="${esc(summary)}"><div class="alert-summary-text">${esc(summary || "—")}</div></td>
    <td class="mono">${esc(ip || "—")}</td>
    <td class="mono">${fmtValue(a.value)}</td>
    <td class="col-src"><span class="source-tag" title="${esc(src)}">${esc(src)}</span></td>
    <td class="col-time">${esc(fmtTime(a.starts_at))}</td>
    <td class="col-dur">${esc(dur)}</td>
    <td class="col-time">${esc(fmtTime(a.last_evaluated_at))}</td>
    <td class="col-act"><button type="button" data-alert-detail="${i}">详情</button></td>
  </tr>`;
    })
    .join("")}</tbody></table>`;
}

function bindAlertTableGrid(container, rows, ingressMap) {
  if (!container || !rows?.length) return;
  container.querySelectorAll("table.alert-table tbody tr[data-alert-idx]").forEach((tr) => {
    tr.addEventListener("contextmenu", (e) => {
      const idx = Number(tr.dataset.alertIdx);
      openAlertContextMenu(e, rows[idx], ingressMap);
    });
  });
  container.querySelectorAll("[data-alert-detail]").forEach((btn) => {
    btn.onclick = () =>
      showAlertDetail(rows[Number(btn.dataset.alertDetail)], ingressMap || {});
  });
}

function alertTable(rows, opts = {}) {
  if (!rows.length) return `<div class="empty">暂无告警事件</div>`;
  const compact = !!opts.compact;
  const ingressMap = opts.ingressMap || {};
  if (!compact) {
    return getAlertView() === "table"
      ? alertTableGrid(rows, ingressMap)
      : alertCards(rows, ingressMap);
  }
  // Overview: always compact cards
  return `<div class="alert-feed compact">${rows
    .slice(0, 8)
    .map((a, i) => {
      const name = alertDisplayName(a);
      const summary = alertSummary(a);
      const src = alertSourceLabel(a, ingressMap);
      return `<article class="alert-card compact status-${esc(a.status)}" data-alert-idx="${i}" tabindex="0" role="button">
        <div class="ac-rail"></div>
        <div class="ac-body">
          <div class="ac-top">
            <div class="ac-title-row">
              <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
              <h3 class="ac-name">${esc(name)}</h3>
            </div>
            <span class="ac-when">${esc(fmtTime(a.last_evaluated_at))}</span>
          </div>
          <p class="ac-summary">${esc(summary || src)}</p>
        </div>
      </article>`;
    })
    .join("")}</div>`;
}

function bindAlertTable(container, rows, ingressMap) {
  bindAlertCards(container, rows, ingressMap);
}

function labelChips(labels, hide = [], max = 6) {
  const entries = Object.entries(labels || {}).filter(([k]) => !hide.includes(k));
  if (!entries.length) return "";
  const shown = entries.slice(0, max);
  const more = entries.length - shown.length;
  return (
    shown
      .map(
        ([k, v]) =>
          `<span class="chip" title="${esc(k)}=${esc(v)}"><b>${esc(k)}</b>${esc(
            String(v).length > 24 ? String(v).slice(0, 24) + "…" : v
          )}</span>`
      )
      .join("") + (more > 0 ? `<span class="chip more">+${more}</span>` : "")
  );
}

function alertSourceLabel(a, ingressMap) {
  const s = (a.labels && a.labels.source) || "";
  const route = ingressMap && ingressMap[a.rule_id];
  if (s.startsWith("ingress:") || route) {
    const kind = s.startsWith("ingress:") ? s.slice("ingress:".length) : route?.kind || "";
    const name = route?.name;
    if (name) return `接入 · ${name}`;
    return kind === "alertmanager" ? "AM 接入" : kind === "kafka" ? "Kafka 接入" : "告警接入";
  }
  return "规则评估";
}

function alertDisplayName(a) {
  const l = a.labels || {};
  return l.alertname || l.bizchainName || l.businessName || l.rule || "未命名告警";
}

function alertSummary(a) {
  const an = a.annotations || {};
  // Prefer summary: Trap 策略「摘要模板」/ 丰富「告警描述」都写 annotations.summary；
  // description 多为策略静态说明，不应盖住真正的告警正文。
  return (
    an.summary ||
    an.description ||
    an.message ||
    an.retMessage ||
    an.title ||
    ""
  );
}

function statusLabel(s) {
  return { firing: "告警中", pending: "等待中", resolved: "已恢复" }[s] || s;
}

function fmtValue(v) {
  if (v == null || v === "") return "—";
  const n = Number(v);
  if (Number.isFinite(n)) {
    return Number.isInteger(n) ? String(n) : n.toFixed(4).replace(/\.?0+$/, "");
  }
  return String(v);
}

function alertDuration(a) {
  if (!a.starts_at) return "—";
  const start = new Date(a.starts_at).getTime();
  const end =
    a.status === "resolved" && a.ends_at
      ? new Date(a.ends_at).getTime()
      : Date.now();
  if (!Number.isFinite(start) || !Number.isFinite(end) || end < start) return "—";
  const sec = Math.floor((end - start) / 1000);
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ${sec % 60}s`;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  if (h < 48) return `${h}h ${m}m`;
  return `${Math.floor(h / 24)}d ${h % 24}h`;
}

function kvTable(obj) {
  const entries = Object.entries(obj || {});
  if (!entries.length) return `<div class="empty" style="padding:8px 0">无</div>`;
  return `<table class="data kv"><tbody>${entries
    .map(
      ([k, v]) =>
        `<tr><th>${esc(k)}</th><td class="mono">${esc(String(v))}</td></tr>`
    )
    .join("")}</tbody></table>`;
}

/** Longest-prefix match: numeric OID → OBJECTS name via policy object_oids (name→base). */
function nameFromObjectOids(oid, objectOids) {
  if (!oid || !objectOids) return "";
  let best = "";
  let bestLen = -1;
  for (const [name, baseRaw] of Object.entries(objectOids)) {
    const base = String(baseRaw || "").trim();
    if (!base) continue;
    if (oid === base || oid.startsWith(base + ".")) {
      if (base.length > bestLen) {
        bestLen = base.length;
        best = name;
      }
    }
  }
  return best;
}

/** Extract OID→value(+name) rows from trap alert annotations. */
function trapVarbindRows(alert, objectOidsFallback) {
  const an = (alert && alert.annotations) || {};
  let nameMap = {};
  if (an.varbind_names) {
    try {
      nameMap =
        typeof an.varbind_names === "string"
          ? JSON.parse(an.varbind_names)
          : an.varbind_names || {};
    } catch (_) {
      nameMap = {};
    }
  }
  for (let i = 0; i < 512; i++) {
    const oid = an[`vb${i}_oid`];
    if (oid == null || oid === "") break;
    const nm = an[`vb${i}_name`];
    if (nm && !nameMap[oid]) nameMap[String(oid)] = String(nm);
  }
  const resolveName = (oid) => {
    if (nameMap[oid]) return String(nameMap[oid]);
    return nameFromObjectOids(oid, objectOidsFallback) || "";
  };
  if (an.varbinds) {
    try {
      const obj = typeof an.varbinds === "string" ? JSON.parse(an.varbinds) : an.varbinds;
      return Object.entries(obj || {}).map(([oid, val]) => ({
        oid,
        val: String(val),
        name: resolveName(oid),
      }));
    } catch (_) {}
  }
  const rows = [];
  for (let i = 0; i < 512; i++) {
    const oid = an[`vb${i}_oid`];
    if (oid == null || oid === "") break;
    const o = String(oid);
    rows.push({
      oid: o,
      val: String(an[`vb${i}_val`] ?? ""),
      name: resolveName(o),
    });
  }
  return rows;
}

async function showTrapRecentDetail(it) {
  if (!it) return;
  const alert = it.alert || {};
  const labels = alert.labels || {};
  const an = alert.annotations || {};
  let objectOids = null;
  const needFallback = () => {
    const rows = trapVarbindRows(alert, null);
    return rows.some((r) => r.oid && !r.name);
  };
  if (needFallback()) {
    const pid = labels.trap_policy_id;
    try {
      if (pid) {
        const p = await api(`/api/policies/${encodeURIComponent(pid)}`);
        objectOids = (p && p.object_oids) || null;
      }
      if (!objectOids || !Object.keys(objectOids).length) {
        const data = await api("/api/policies");
        const list = (data && data.items) || data || [];
        const oid = it.trap_oid || labels.trap_oid || "";
        const hit = Array.isArray(list)
          ? list.find((p) => p && p.trap_oid === oid && p.object_oids)
          : null;
        if (hit) objectOids = hit.object_oids;
      }
    } catch (_) {}
  }
  const vbs = trapVarbindRows(alert, objectOids);
  const named = vbs.filter((r) => r.name).length;
  openModal(
    `
    <div class="modal-head">
      <h3>Trap 详情</h3>
      <p class="desc">
        <span class="mono">${esc(it.peer || labels.ip || "—")}</span>
        · ${esc(it.alertname || labels.alertname || "—")}
        <span class="hint" style="margin-left:8px">${esc(fmtTime(it.at))}</span>
      </p>
    </div>
    <div class="modal-body">
      <div class="field"><label>Trap OID</label>
        <div class="summary-box mono">${esc(it.trap_oid || labels.trap_oid || "—")}</div>
      </div>
      <div class="field"><label>摘要</label>
        <div class="summary-box">${esc(an.summary || "—")}</div>
      </div>
      <div class="field"><label>Varbinds（OID / 变量名 / 值，共 ${vbs.length}${
        named ? `，已识别 ${named}` : ""
      }）</label>
        ${
          vbs.length
            ? `<div class="alert-table-scroll"><table class="data"><thead><tr><th style="width:3rem">#</th><th>变量名</th><th>OID</th><th>值</th></tr></thead>
              <tbody>${vbs
                .map(
                  (r, i) =>
                    `<tr><td>${i}</td><td class="mono">${esc(r.name || "—")}</td><td class="mono">${esc(
                      r.oid
                    )}</td><td class="mono">${esc(r.val)}</td></tr>`
                )
                .join("")}</tbody></table></div>`
            : `<div class="empty" style="padding:8px 0">无 varbind</div>`
        }
      </div>
      <div class="field"><label>Labels</label>${kvTable(labels)}</div>
      <details style="margin-top:12px">
        <summary class="hint" style="cursor:pointer">原始告警 JSON</summary>
        <pre class="mono" style="white-space:pre-wrap;font-size:0.78rem;max-height:280px;overflow:auto;margin-top:8px">${esc(
          JSON.stringify(alert, null, 2)
        )}</pre>
      </details>
    </div>
    <div class="modal-foot"><button type="button" class="ghost" id="trap-detail-close">关闭</button></div>
  `,
    { xl: true }
  );
  const closeBtn = document.getElementById("trap-detail-close");
  if (closeBtn) closeBtn.onclick = () => closeModal();
}

async function showAlertDetail(a, ingressMap = {}) {
  if (!a) return;
  const name = alertDisplayName(a);
  const srcLabel = alertSourceLabel(a, ingressMap);
  const ip = alertTargetIp(a);
  let notifies = [];
  try {
    notifies = await api(`/api/alerts/${a.id}/notifies`);
  } catch {
    notifies = [];
  }
  const notifyHtml = notifies.length
    ? `<table class="data"><thead><tr><th>时间</th><th>边沿</th><th>结果</th><th>内容摘要</th><th>错误</th><th></th></tr></thead>
       <tbody>${notifies
         .map(
           (n, i) => `<tr>
         <td>${esc(fmtTime(n.created_at))}</td>
         <td>${esc(
           n.transition === "became_firing"
             ? "触发通知"
             : n.transition === "became_resolved"
             ? "恢复通知"
             : n.transition
         )}</td>
         <td><span class="badge ${n.success ? "on" : "firing"}">${
           n.success ? "成功" : "失败"
         }</span></td>
         <td class="mono">${esc(
           String(n.body || "")
             .replace(/\s+/g, " ")
             .trim()
             .slice(0, 48) || "—"
         )}${String(n.body || "").length > 48 ? "…" : ""}</td>
         <td class="mono">${esc(n.error || "—")}</td>
         <td class="actions"><button type="button" data-nlog="${i}">查看</button></td>
       </tr>`
         )
         .join("")}</tbody></table>`
    : `<div class="empty" style="padding:8px 0">暂无通知记录（可能未绑定渠道，或尚未发生状态边沿）</div>`;

  const trapVbs = trapVarbindRows(a);
  const anForTable = { ...(a.annotations || {}) };
  delete anForTable.varbinds;
  delete anForTable.varbind_count;
  Object.keys(anForTable).forEach((k) => {
    if (/^vb\d+_(oid|val)$/.test(k)) delete anForTable[k];
  });
  const trapVbHtml = trapVbs.length
    ? `<div class="field">
        <label>SNMP Varbinds（完整 OID / 值，共 ${trapVbs.length}）</label>
        <div class="alert-table-scroll"><table class="data"><thead><tr><th style="width:3rem">#</th><th>OID</th><th>值</th></tr></thead>
          <tbody>${trapVbs
            .map(
              (r, i) =>
                `<tr><td>${i}</td><td class="mono">${esc(r.oid)}</td><td class="mono">${esc(
                  r.val
                )}</td></tr>`
            )
            .join("")}</tbody></table></div>
      </div>`
    : "";

  openModal(`
    <div class="modal-head">
      <h3>${esc(name)}</h3>
      <p class="desc">
        <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
        <span class="sev sev-${esc(a.severity)}" style="margin-left:8px">${esc(severityLabel(a.severity))}</span>
        <span class="source-tag" style="margin-left:8px">${esc(srcLabel)}</span>
        ${ip ? `<span class="ac-ip" style="margin-left:8px">IP ${esc(ip)}</span>` : ""}
      </p>
    </div>
    <div class="modal-body alert-detail">
      <div class="detail-grid">
        <div><label>告警 IP</label><div class="mono">${esc(ip || "—")}</div></div>
        <div><label>当前值</label><div class="mono">${fmtValue(a.value)}</div></div>
        <div><label>持续时长</label><div>${esc(alertDuration(a))}</div></div>
        <div><label>开始时间</label><div>${esc(fmtTime(a.starts_at))}</div></div>
        <div><label>结束时间</label><div>${esc(a.ends_at ? fmtTime(a.ends_at) : "—")}</div></div>
        <div><label>最后更新</label><div>${esc(fmtTime(a.last_evaluated_at))}</div></div>
        <div><label>Pending 起</label><div>${esc(
          a.pending_since ? fmtTime(a.pending_since) : "—"
        )}</div></div>
        <div><label>已通知触发</label><div>${a.notified_firing ? "是" : "否"}</div></div>
        <div><label>已通知恢复</label><div>${a.notified_resolved ? "是" : "否"}</div></div>
      </div>
      <div class="field">
        <label>告警描述</label>
        <div class="summary-box">${esc(alertSummary(a) || "—")}</div>
        ${kvTable(anForTable)}
      </div>
      ${trapVbHtml}
      <div class="field">
        <label>标签</label>
        ${kvTable(a.labels)}
      </div>
      <div class="field">
        <label>告警标识</label>
        <div class="mono" style="word-break:break-all">${esc(a.fingerprint)}</div>
      </div>
      <div class="field">
        <label>通知记录</label>
        ${notifyHtml}
      </div>
      <div class="field">
        <label>事件 ID / 规则·路由 ID</label>
        <div class="mono" style="font-size:12px">alert: ${esc(a.id)}<br/>rule/route: ${esc(
          a.rule_id
        )}</div>
      </div>
    </div>
    <div class="modal-actions">
      <button type="button" class="ghost" id="m-silence">据此静默</button>
      <button type="button" class="primary" id="m-close">关闭</button>
    </div>`, { xl: true });
  document.getElementById("m-close").onclick = closeModal;
  document.querySelectorAll("[data-nlog]").forEach((b) => {
    b.onclick = () => {
      const n = notifies[Number(b.dataset.nlog)];
      if (!n) return;
      showNotifyLogDetail({
        ...n,
        channel_name: n.channel_name || "",
      });
    };
  });
  document.getElementById("m-silence").onclick = () => {
    closeModal();
    silenceFromAlert(a);
  };
}


  async function renderAlerts(root) {
    const f = state.alertFilters || {
      status: "",
      severity: "",
      source: "",
      q: "",
      ip: "",
      store: "",
      page: 1,
    };
    setActions(`<button class="ghost" id="btn-refresh">刷新</button>`);

    const load = async (opts = {}) => {
      const soft = !!opts.soft;
      if (soft) {
        const ae = document.activeElement;
        if (ae && (ae.id === "alert-q" || ae.id === "alert-ip")) return;
      }

      const status = state.alertFilters?.status || "";
      const severity =
        root.querySelector("#alert-sev")?.value ?? state.alertFilters?.severity ?? "";
      const source =
        root.querySelector("#alert-source")?.value ?? state.alertFilters?.source ?? "";
      const q = (root.querySelector("#alert-q")?.value ?? state.alertFilters?.q ?? "").trim();
      const ip = (root.querySelector("#alert-ip")?.value ?? state.alertFilters?.ip ?? "").trim();
      const store =
        root.querySelector("#alert-store")?.value ?? state.alertFilters?.store ?? "";
      let page = Number(state.alertFilters?.page) || 1;
      if (page < 1) page = 1;
      const limit = ALERT_PAGE_SIZE;
      state.alertFilters = { status, severity, source, q, ip, store, page };

      const params = new URLSearchParams();
      if (status) params.set("status", status);
      if (severity) params.set("severity", severity);
      if (source) params.set("source", source);
      if (q) params.set("q", q);
      if (ip) params.set("ip", ip);
      if (store) params.set("store", store);
      params.set("page", String(page));
      params.set("limit", String(limit));

      const [data, ingressRows] = await Promise.all([
        api("/api/alerts?" + params.toString()),
        api("/api/ingress").catch(() => []),
      ]);
      const list = normalizeAlertList(data);
      const rows = list.items;
      const total = list.total;
      const nFire = list.status_counts.firing;
      const nPend = list.status_counts.pending;
      const nRes = list.status_counts.resolved;
      const ingressMap = Object.fromEntries((ingressRows || []).map((r) => [r.id, r]));
      const pages = Math.max(1, Math.ceil(total / limit) || 1);
      if (page > pages) {
        state.alertFilters = { ...(state.alertFilters || {}), page: pages };
        return load(opts);
      }

      const view = getAlertView();
      const auto = getAlertAutoRefresh();
      root.innerHTML = `
        <div class="alert-toolbar">
          <div class="alert-tabs" role="tablist">
            <button type="button" class="alert-tab ${!status ? "on" : ""}" data-st="">全部</button>
            <button type="button" class="alert-tab firing ${status === "firing" ? "on" : ""}" data-st="firing">告警中 <em>${nFire}</em></button>
            <button type="button" class="alert-tab ${status === "pending" ? "on" : ""}" data-st="pending">等待 <em>${nPend}</em></button>
            <button type="button" class="alert-tab ok ${status === "resolved" ? "on" : ""}" data-st="resolved">已恢复 <em>${nRes}</em></button>
          </div>
          <div class="alert-filters">
            <div class="view-switch" role="group" aria-label="列表样式">
              <button type="button" class="view-btn ${view === "cards" ? "on" : ""}" data-view="cards" title="卡片列表">卡片</button>
              <button type="button" class="view-btn ${view === "table" ? "on" : ""}" data-view="table" title="表格列表">表格</button>
            </div>
            <input id="alert-q" placeholder="搜索名称、描述…" value="${esc(q)}" />
            <input id="alert-ip" class="alert-ip-input" placeholder="IP / 主机" value="${esc(ip)}" title="按告警 IP / instance / host 筛选" />
            <select id="alert-sev">
              <option value="" ${!severity ? "selected" : ""}>全部级别</option>
              ${severityOptions(severity, "")}
            </select>
            <select id="alert-source">
              <option value="">来源</option>
              <option value="ingress" ${source === "ingress" ? "selected" : ""}>告警接入</option>
              <option value="rule" ${source === "rule" ? "selected" : ""}>规则</option>
            </select>
            <select id="alert-store" title="列表范围">
              <option value="" ${!store ? "selected" : ""}>列表·默认</option>
              <option value="mysql" ${store === "mysql" ? "selected" : ""}>最近事件</option>
              <option value="es" ${store === "es" ? "selected" : ""}>历史事件</option>
            </select>
            <label class="alert-auto" title="定时刷新列表">
              <span>自动刷新</span>
              <select id="alert-auto">
                <option value="0" ${auto === 0 ? "selected" : ""}>关</option>
                <option value="15" ${auto === 15 ? "selected" : ""}>15 秒</option>
                <option value="30" ${auto === 30 ? "selected" : ""}>30 秒</option>
                <option value="60" ${auto === 60 ? "selected" : ""}>60 秒</option>
              </select>
            </label>
          </div>
        </div>
        ${
          rows.length
            ? view === "table"
              ? `<div class="panel alert-table-scroll">${alertTableGrid(rows, ingressMap)}</div>`
              : alertCards(rows, ingressMap)
            : `<div class="panel empty">
                暂无告警。去「告警接入」试推送，或等待规则触发。
                <div style="margin-top:12px"><button class="primary" id="go-ingress">去告警接入</button></div>
              </div>`
        }
        ${alertPagerHtml(page, pages, total, limit)}`;

      root.querySelectorAll(".alert-tab").forEach((tab) => {
        tab.onclick = () => {
          state.alertFilters = {
            ...(state.alertFilters || {}),
            status: tab.dataset.st || "",
            page: 1,
          };
          load();
        };
      });
      root.querySelectorAll(".view-btn").forEach((btn) => {
        btn.onclick = () => {
          setAlertView(btn.dataset.view);
          load();
        };
      });
      if (view === "table") bindAlertTableGrid(root, rows, ingressMap);
      else bindAlertCards(root, rows, ingressMap);
      const go = document.getElementById("go-ingress");
      if (go) go.onclick = () => navigate("ingress");
      bindAlertPager(root, () => load());

      let t;
      const qEl = root.querySelector("#alert-q");
      qEl.oninput = () => {
        clearTimeout(t);
        t = setTimeout(() => {
          state.alertFilters = {
            ...(state.alertFilters || {}),
            q: qEl.value.trim(),
            page: 1,
          };
          load();
        }, 280);
      };
      const ipEl = root.querySelector("#alert-ip");
      let tip;
      ipEl.oninput = () => {
        clearTimeout(tip);
        tip = setTimeout(() => {
          state.alertFilters = {
            ...(state.alertFilters || {}),
            ip: ipEl.value.trim(),
            page: 1,
          };
          load();
        }, 280);
      };
      root.querySelector("#alert-sev").onchange = (e) => {
        state.alertFilters = {
          ...(state.alertFilters || {}),
          severity: e.target.value,
          page: 1,
        };
        load();
      };
      root.querySelector("#alert-source").onchange = (e) => {
        state.alertFilters = {
          ...(state.alertFilters || {}),
          source: e.target.value,
          page: 1,
        };
        load();
      };
      root.querySelector("#alert-store").onchange = (e) => {
        state.alertFilters = {
          ...(state.alertFilters || {}),
          store: e.target.value,
          page: 1,
        };
        load();
      };
      root.querySelector("#alert-auto").onchange = (e) => {
        setAlertAutoRefresh(e.target.value);
        scheduleAlertsAutoRefresh(() => load({ soft: true }));
      };
    };

    document.getElementById("btn-refresh").onclick = () => load();
    state.alertFilters = { ...f, page: Number(f.page) || 1, ip: f.ip || "" };
    await load();
    scheduleAlertsAutoRefresh(() => load({ soft: true }));
  }

  return {
    renderAlerts,
    alertTable,
    bindAlertTable,
    showAlertDetail,
    showTrapRecentDetail,
    scheduleAlertsAutoRefresh,
    stopAlertsTimer: _stop,
  };
}
