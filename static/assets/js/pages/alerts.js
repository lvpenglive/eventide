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
    editMaintenance,
    showNotifyLogDetail,
  } = deps;

  function canWriteAlerts() {
    const perms = (state.me && state.me.permissions) || [];
    return perms.includes("*") || perms.includes("alerts:write");
  }

  function alertIsAcked(a) {
    return !!(a && a.acknowledged_at);
  }

  function ackBadgeHtml(a) {
    if (!alertIsAcked(a)) return "";
    const who = a.assignee || a.acknowledged_by || "已确认";
    return `<span class="badge ack" title="确认人 ${esc(a.acknowledged_by || "")} · ${esc(
      fmtTime(a.acknowledged_at)
    )}">已接手 · ${esc(who)}</span>`;
  }

  function maintenanceWindowsCache() {
    return state.cache?.maintenanceWindows || [];
  }

  function maintenanceHitsForAlert(a, windows, now = Date.now()) {
    return (windows || []).filter((w) => {
      if (w.enabled === false) return false;
      const s = new Date(w.starts_at).getTime();
      const e = new Date(w.ends_at).getTime();
      if (!(s <= now && now < e)) return false;
      if (w.rule_id && w.rule_id !== a.rule_id) return false;
      const labels = a.labels || {};
      for (const [k, v] of Object.entries(w.matchers || {})) {
        if (labels[k] !== v) return false;
      }
      return true;
    });
  }

  function mwBadgeHtml(a, windows) {
    const hits = maintenanceHitsForAlert(a, windows ?? maintenanceWindowsCache());
    if (!hits.length) return "";
    const title = hits.map((h) => h.name || h.comment || h.id).join(" · ");
    return `<span class="badge mw" title="${esc(title)}">维护中</span>`;
  }

  function escalatedBadgeHtml(a) {
    if (!a?.escalated_at) return "";
    return `<span class="badge escalated" title="升级于 ${esc(
      fmtTime(a.escalated_at)
    )}">已升级</span>`;
  }

  function maintenanceFromAlert(a) {
    if (!a) return;
    const matchers = { ...(a.labels || {}) };
    delete matchers.source;
    editMaintenance({
      name: `维护 ${alertDisplayName(a)}`,
      comment: `来自告警 ${alertDisplayName(a)}`,
      matchers,
      rule_id: null,
    });
  }

  async function ackAlert(a, { comment, assignee } = {}) {
    const body = {};
    if (comment) body.comment = comment;
    if (assignee) body.assignee = assignee;
    return api(`/api/alerts/${a.id}/ack`, {
      method: "POST",
      body: JSON.stringify(body),
    });
  }

  async function unackAlert(a) {
    return api(`/api/alerts/${a.id}/unack`, { method: "POST", body: "{}" });
  }

  function openAckModal(a, onDone) {
    const me = (state.me && (state.me.username || state.me.display_name)) || "";
    openModal(`
      <div class="modal-head">
        <h3>确认接手</h3>
        <p class="desc">标记有人在处理；不影响告警状态与通知静默。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>接手人</label>
          <input name="assignee" value="${esc(a.assignee || me)}" placeholder="默认当前用户" /></div>
        <div class="field"><label>备注（可选）</label>
          <textarea name="comment" rows="3" placeholder="处理说明…">${esc(
            a.ack_comment || ""
          )}</textarea></div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">确认接手</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(e.target);
      try {
        const updated = await ackAlert(a, {
          assignee: String(fd.get("assignee") || "").trim(),
          comment: String(fd.get("comment") || "").trim(),
        });
        closeModal();
        toast("已确认接手");
        if (onDone) onDone(updated || a);
        else navigate("alerts");
      } catch (err) {
        toast(err.message || "操作失败", true);
      }
    };
  }

  async function closeAlertApi(a, { comment, notify = true } = {}) {
    return api(`/api/alerts/${a.id}/close`, {
      method: "POST",
      body: JSON.stringify({ comment, notify }),
    });
  }

  function openCloseModal(a) {
    openModal(`
      <div class="modal-head">
        <h3>关闭告警</h3>
        <p class="desc">人工强制恢复（适用于无 recover 的来源）。同指纹再次触发会作为新问题。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>关闭原因（必填）</label>
          <textarea name="comment" rows="3" required placeholder="例如：误报 / 已线下处理 / 设备更换…"></textarea></div>
        <label class="check-row"><input type="checkbox" name="notify" checked /> <span>发送恢复通知到绑定渠道</span></label>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">确认关闭</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(e.target);
      const comment = String(fd.get("comment") || "").trim();
      if (!comment) {
        toast("请填写关闭原因", true);
        return;
      }
      try {
        await closeAlertApi(a, {
          comment,
          notify: !!e.target.querySelector('[name="notify"]')?.checked,
        });
        closeModal();
        toast("告警已关闭");
        navigate("alerts");
      } catch (err) {
        toast(err.message || "关闭失败", true);
      }
    };
  }

  function selectedAlertIds() {
    if (!state.alertSelected) state.alertSelected = new Set();
    return state.alertSelected;
  }

  function clearAlertSelection() {
    selectedAlertIds().clear();
  }

  function pruneAlertSelection(rows) {
    const sel = selectedAlertIds();
    const live = new Set((rows || []).map((a) => a.id).filter(Boolean));
    for (const id of [...sel]) {
      if (!live.has(id)) sel.delete(id);
    }
  }

  function toastBatchResult(label, res) {
    const ok = (res && res.ok) || [];
    const failed = (res && res.failed) || [];
    for (const id of ok) selectedAlertIds().delete(id);
    if (!failed.length) {
      toast(`${label} ${ok.length} 条`);
      return;
    }
    const sample = failed
      .slice(0, 2)
      .map((f) => f.error || "失败")
      .join("；");
    toast(
      `${label}成功 ${ok.length}，失败 ${failed.length}${sample ? `：${sample}` : ""}`,
      true
    );
  }

  function openBatchAckModal(ids, onDone) {
    const me = (state.me && (state.me.username || state.me.display_name)) || "";
    openModal(`
      <div class="modal-head">
        <h3>批量接手</h3>
        <p class="desc">将为选中的 ${ids.length} 条告警标记接手；不影响状态与通知静默。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>接手人</label>
          <input name="assignee" value="${esc(me)}" placeholder="默认当前用户" /></div>
        <div class="field"><label>备注（可选）</label>
          <textarea name="comment" rows="3" placeholder="统一处理说明…"></textarea></div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">确认接手</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(e.target);
      try {
        const res = await api("/api/alerts/batch/ack", {
          method: "POST",
          body: JSON.stringify({
            ids,
            assignee: String(fd.get("assignee") || "").trim(),
            comment: String(fd.get("comment") || "").trim(),
          }),
        });
        closeModal();
        toastBatchResult("已接手", res);
        if (onDone) onDone();
        else navigate("alerts");
      } catch (err) {
        toast(err.message || "批量接手失败", true);
      }
    };
  }

  function openBatchCloseModal(ids, onDone) {
    openModal(`
      <div class="modal-head">
        <h3>批量关闭</h3>
        <p class="desc">将关闭选中的 ${ids.length} 条未恢复告警。同指纹再次触发会作为新问题。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>关闭原因（必填）</label>
          <textarea name="comment" rows="3" required placeholder="例如：批量误报 / 变更窗口已结束…"></textarea></div>
        <label class="check-row"><input type="checkbox" name="notify" /> <span>发送恢复通知到绑定渠道（批量默认关闭，避免刷屏）</span></label>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">确认关闭</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(e.target);
      const comment = String(fd.get("comment") || "").trim();
      if (!comment) {
        toast("请填写关闭原因", true);
        return;
      }
      try {
        const res = await api("/api/alerts/batch/close", {
          method: "POST",
          body: JSON.stringify({
            ids,
            comment,
            notify: !!e.target.querySelector('[name="notify"]')?.checked,
          }),
        });
        closeModal();
        toastBatchResult("已关闭", res);
        if (onDone) onDone();
        else navigate("alerts");
      } catch (err) {
        toast(err.message || "批量关闭失败", true);
      }
    };
  }

  function alertBulkBarHtml(canWrite) {
    const n = selectedAlertIds().size;
    if (!n || !canWrite) return "";
    return `<div class="alert-bulk-bar" role="region" aria-label="批量操作">
      <span class="alert-bulk-meta">已选 <strong>${n}</strong> 条</span>
      <div class="alert-bulk-actions">
        <button type="button" class="primary" id="btn-bulk-ack">批量接手</button>
        <button type="button" class="danger" id="btn-bulk-close">批量关闭</button>
        <button type="button" class="ghost" id="btn-bulk-clear">取消选择</button>
      </div>
    </div>`;
  }

  function bindAlertBulkBar(root, rows, reload) {
    const ackBtn = root.querySelector("#btn-bulk-ack");
    const closeBtn = root.querySelector("#btn-bulk-close");
    const clearBtn = root.querySelector("#btn-bulk-clear");
    if (clearBtn) {
      clearBtn.onclick = () => {
        clearAlertSelection();
        reload();
      };
    }
    const idsOnPage = new Set((rows || []).map((a) => a.id));
    const picked = () => {
      const ids = [...selectedAlertIds()].filter((id) => idsOnPage.has(id));
      if (ids.length > 50) {
        toast("单次最多处理 50 条", true);
        return ids.slice(0, 50);
      }
      return ids;
    };
    if (ackBtn) {
      ackBtn.onclick = () => {
        const ids = picked();
        if (!ids.length) {
          toast("请先勾选告警", true);
          return;
        }
        openBatchAckModal(ids, reload);
      };
    }
    if (closeBtn) {
      closeBtn.onclick = () => {
        const ids = picked();
        if (!ids.length) {
          toast("请先勾选告警", true);
          return;
        }
        openBatchCloseModal(ids, reload);
      };
    }
  }

  function bindAlertSelection(container, rows, reload) {
    if (!container || !rows?.length) return;
    const sel = selectedAlertIds();
    const paintBulk = () => {
      const root = container.closest("#page-root") || container;
      const barHost = root.querySelector("[data-alert-bulk-host]");
      if (!barHost) return;
      barHost.innerHTML = alertBulkBarHtml(canWriteAlerts());
      bindAlertBulkBar(root, rows, reload);
    };
    const syncSelectAll = () => {
      const all = container.querySelector("#alert-select-all");
      if (!all) return;
      const ids = rows.map((a) => a.id).filter(Boolean);
      const n = ids.filter((id) => sel.has(id)).length;
      all.checked = ids.length > 0 && n === ids.length;
      all.indeterminate = n > 0 && n < ids.length;
    };
    container.querySelectorAll(".alert-select").forEach((cb) => {
      cb.addEventListener("click", (e) => e.stopPropagation());
      cb.addEventListener("change", (e) => {
        e.stopPropagation();
        const id = cb.dataset.alertId;
        if (!id) return;
        if (cb.checked) sel.add(id);
        else sel.delete(id);
        const card = cb.closest(".alert-card, tr");
        if (card) card.classList.toggle("is-selected", cb.checked);
        syncSelectAll();
        paintBulk();
      });
    });
    const all = container.querySelector("#alert-select-all");
    if (all) {
      all.addEventListener("change", () => {
        rows.forEach((a) => {
          if (!a?.id) return;
          if (all.checked) sel.add(a.id);
          else sel.delete(a.id);
        });
        container.querySelectorAll(".alert-select").forEach((cb) => {
          cb.checked = all.checked;
          const card = cb.closest(".alert-card, tr");
          if (card) card.classList.toggle("is-selected", all.checked);
        });
        all.indeterminate = false;
        paintBulk();
      });
      syncSelectAll();
    }
  }

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
  // Do not treat host/hostname as IP — those are Node names.
  return l.alertIp || l.ip || l.ipaddr || l.instance || "";
}

function alertHostname(a) {
  const l = a.labels || {};
  const direct =
    l.hostname || l.host || l["主机名"] || l.Node || l.node || l.nodename || "";
  if (direct) return String(direct).trim();
  for (const [k, v] of Object.entries(l)) {
    if (!v) continue;
    const key = String(k);
    if (
      key.endsWith(".主机名") ||
      key.endsWith(".hostname") ||
      key === "主机名" ||
      /(^|[.])host(name)?$/i.test(key)
    ) {
      const s = String(v).trim();
      if (s) return s;
    }
  }
  return "";
}

function alertGroup(a) {
  const l = a.labels || {};
  return (
    l.alertgroup ||
    l.AlertGroup ||
    l.alert_group ||
    l.alertname ||
    ""
  );
}
function alertInstance(a) {
  const l = a.labels || {};
  const s = String(l.instance || l.endpoint || l.svc || "").trim();
  if (!s) return "";
  const ip = alertTargetIp(a);
  const host = alertHostname(a);
  if (s === ip || s === host) return "";
  if (/^\d+\.\d+\.\d+\.\d+$/.test(s) && s === ip) return "";
  return s;
}


function alertKey(a) {
  const l = a.labels || {};
  return (
    l.alertkey ||
    l.AlertKey ||
    l.alert_key ||
    l.ifDescr ||
    l.ifName ||
    l.trap_oid ||
    l.instance ||
    ""
  );
}

function alertTally(a) {
  const n = Number(a?.tally);
  return Number.isFinite(n) && n > 0 ? Math.floor(n) : 1;
}

function alertNoteHint(a) {
  const parts = [];
  if (a?.ack_comment) parts.push(`接手：${a.ack_comment}`);
  if (a?.close_comment) parts.push(`关闭：${a.close_comment}`);
  if (!parts.length) return "";
  return parts.join("\n");
}

/** Merge labels + annotations for CMDB/enrich fields (annotations win on clash). */
function alertFieldBag(a) {
  return { ...(a?.labels || {}), ...(a?.annotations || {}) };
}

function splitContactValues(raw) {
  return String(raw || "")
    .split(/[,;/|、，]+/)
    .map((s) => s.trim())
    .filter(Boolean);
}

function normalizeFieldKey(k) {
  return String(k || "")
    .trim()
    .toLowerCase()
    .replace(/[\s_-]+/g, "");
}

/** Find first bag value whose key equals or ends with one of the aliases. */
function findFieldRaw(bag, aliases) {
  const want = aliases.map(normalizeFieldKey);
  for (const [k, v] of Object.entries(bag || {})) {
    if (v == null || String(v).trim() === "") continue;
    const nk = normalizeFieldKey(k);
    const hit = want.some((w) => nk === w || nk.endsWith("." + w) || nk.endsWith(w));
    if (hit) return String(v).trim();
  }
  return "";
}

/**
 * Contacts for on-call calling — from enrich/CMDB lookup columns.
 * Supports multiple values (comma / 、 / ; separated) and role pairs (硬件/应用).
 */
function alertContacts(a) {
  const bag = alertFieldBag(a);
  const roles = [
    {
      role: "硬件",
      names: ["硬件负责人", "hw_owner", "hardware_owner", "hwowner", "hwcontact"],
      phones: ["硬件电话", "硬件手机", "hw_phone", "hardware_phone", "hwphone", "hwmobile"],
    },
    {
      role: "应用",
      names: ["应用负责人", "app_owner", "application_owner", "appowner", "appcontact"],
      phones: ["应用电话", "应用手机", "app_phone", "application_phone", "appphone", "appmobile"],
    },
    {
      role: "",
      names: [
        "联系人",
        "contacts",
        "contact",
        "owner",
        "值班人",
        "负责人",
        "oncall",
        "on_call",
      ],
      phones: [
        "手机号",
        "手机",
        "电话",
        "联系电话",
        "phone",
        "phones",
        "mobile",
        "tel",
        "telephone",
        "cellphone",
      ],
    },
  ];
  const out = [];
  const seen = new Set();
  for (const r of roles) {
    const names = splitContactValues(findFieldRaw(bag, r.names));
    const phones = splitContactValues(findFieldRaw(bag, r.phones));
    if (!names.length && !phones.length) continue;
    const n = Math.max(names.length, phones.length);
    for (let i = 0; i < n; i++) {
      const name = names[i] || "";
      const phone = phones[i] || "";
      if (!name && !phone) continue;
      const key = `${r.role}|${name}|${phone}`;
      if (seen.has(key)) continue;
      seen.add(key);
      out.push({ role: r.role, name, phone });
    }
  }
  return out;
}

function telHref(phone) {
  const digits = String(phone || "").replace(/[^\d+]/g, "");
  return digits ? `tel:${digits}` : "";
}

function alertContactsHtml(a, { compact = false } = {}) {
  const list = alertContacts(a);
  if (!list.length) {
    return compact ? "" : `<span class="hint">—</span>`;
  }
  return `<div class="alert-contacts${compact ? " compact" : ""}">${list
    .map((c) => {
      const role = c.role
        ? `<span class="ac-role">${esc(c.role)}</span>`
        : "";
      const name = c.name ? `<span class="ac-person">${esc(c.name)}</span>` : "";
      const href = telHref(c.phone);
      const phone = c.phone
        ? href
          ? `<a class="ac-phone" href="${esc(href)}" title="拨打 ${esc(
              c.phone
            )}">${esc(c.phone)}</a>`
          : `<span class="ac-phone">${esc(c.phone)}</span>`
        : "";
      return `<div class="ac-contact-row">${role}${name}${
        name && phone ? `<span class="dot">·</span>` : ""
      }${phone}</div>`;
    })
    .join("")}</div>`;
}

function alertCards(rows, ingressMap) {
  const sel = typeof selectedAlertIds === "function" ? selectedAlertIds() : new Set();
  const canWrite = typeof canWriteAlerts === "function" && canWriteAlerts();
  return `<div class="alert-feed">${
    canWrite
      ? `<div class="alert-select-all-row"><label><input type="checkbox" id="alert-select-all" /> 本页全选</label></div>`
      : ""
  }${rows
    .map((a, i) => {
      const name = alertDisplayName(a);
      const summary = alertSummary(a);
      const preview = alertSummaryPreview(a, 160);
      const dur = alertDuration(a);
      const src = alertSourceLabel(a, ingressMap);
      const ip = alertTargetIp(a);
      const host = alertHostname(a);
      const inst = alertInstance(a);
      const group = alertGroup(a);
      const key = alertKey(a);
      const note = alertNoteHint(a);
      const tally = alertTally(a);
      const contactsHtml = alertContactsHtml(a, { compact: true });
      const checked = sel.has(a.id);
      const chips = labelChips(
        a.labels,
        [
          "alertname",
          "severity",
          "source",
          "alertIp",
          "ip",
          "ipaddr",
          "instance",
          "host",
          "hostname",
          "主机名",
          "alertgroup",
          "AlertGroup",
          "alert_group",
          "alertkey",
          "AlertKey",
          "alert_key",
        ],
        3
      );
      const tallyBadge = tally > 1
        ? `<span class="badge tally" title="重复次数 ×${tally}">×${tally}</span>`
        : "";
      // Primary info row: 定位 + 程度
      const primaryBits = [];
      if (ip) primaryBits.push(`<span class="ac-tag mono" title="告警 IP">IP ${esc(ip)}</span>`);
      if (host && host !== ip) primaryBits.push(`<span class="ac-tag" title="主机名">主机 ${esc(host)}</span>`);
      if (inst) primaryBits.push(`<span class="ac-tag mono" title="实例 / 端口">实例 ${esc(inst)}</span>`);
      primaryBits.push(`<span class="ac-tag emph" title="已持续">⏱ ${esc(dur)}</span>`);
      // Secondary info row: 来源 / 时间 / 组键 / 评估
      const secBits = [];
      secBits.push(`<span>${esc(src)}</span>`);
      secBits.push(`<span title="开始时间">起 ${esc(fmtTime(a.starts_at))}</span>`);
      secBits.push(`<span title="末次发生">末 ${esc(fmtTime(a.last_occurrence_at || a.starts_at))}</span>`);
      if (group) secBits.push(`<span title="AlertGroup">组 ${esc(group)}</span>`);
      if (key && key !== group) secBits.push(`<span class="mono" title="AlertKey">键 ${esc(key)}</span>`);
      secBits.push(`<span title="最后评估">评 ${esc(fmtTime(a.last_evaluated_at))}</span>`);
      const secHtml = secBits.length
        ? `<div class="ac-meta ac-meta-sec">${secBits.join(' <span class="dot">·</span> ')}</div>`
        : "";
      // Footer tags (handled 接手/维护/升级/备注/次数 全部后置)
      const footerTags = [
        ackBadgeHtml(a),
        mwBadgeHtml(a),
        escalatedBadgeHtml(a),
        note ? `<span class="badge note" title="${esc(note)}">备注</span>` : "",
        tallyBadge,
      ].filter(Boolean);
      const footerHtml = footerTags.length
        ? `<div class="ac-footer-tags">${footerTags.join("")}</div>`
        : "";
      return `<article class="alert-card status-${esc(a.status)} sev-${esc(
        a.severity
      )}${checked ? " is-selected" : ""}${alertIsAcked(a) ? " is-acked" : ""}" data-alert-idx="${i}" tabindex="0" role="button">
        <div class="ac-rail" aria-hidden="true"></div>
        ${
          canWrite
            ? `<label class="ac-check"><input type="checkbox" class="alert-select" data-alert-id="${esc(
                a.id
              )}" ${checked ? "checked" : ""} /></label>`
            : ""
        }
        <div class="ac-body">
          <div class="ac-top">
            <div class="ac-headline">
              <h3 class="ac-name" title="${esc(name)}">${esc(name)}</h3>
              <div class="ac-tags">
                <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
                <span class="sev sev-${esc(a.severity)}">${esc(severityLabel(a.severity))}</span>
              </div>
            </div>
            <button type="button" class="ghost ac-detail" data-alert-detail="${i}">详情</button>
          </div>
          <div class="ac-meta ac-meta-pri">${primaryBits.join("")}</div>
          <p class="ac-summary" title="${esc(summary || "")}">${esc(preview || "无描述")}</p>
          ${contactsHtml ? `<div class="ac-contacts-line">${contactsHtml}</div>` : ""}
          ${secHtml}
          <div class="alert-labels">${chips}</div>
          ${footerHtml}
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
      label: alertIsAcked(a) ? "取消接手" : "确认接手",
      disabled: !canWriteAlerts(),
      onClick: async () => {
        try {
          if (alertIsAcked(a)) {
            await unackAlert(a);
            toast("已取消接手");
            navigate("alerts");
          } else {
            openAckModal(a);
          }
        } catch (err) {
          toast(err.message || "操作失败", true);
        }
      },
    },
    {
      label: "关闭告警",
      disabled: !canWriteAlerts() || a.status === "resolved",
      onClick: () => openCloseModal(a),
    },
    {
      label: "据此静默",
      onClick: () => silenceFromAlert(a),
    },
    {
      label: "据此开维护",
      onClick: () => maintenanceFromAlert(a),
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

function bindAlertCards(container, rows, ingressMap, reload) {
  if (!container || !rows?.length) return;
  bindAlertSelection(container, rows, reload);
  const open = (i) => showAlertDetail(rows[i], ingressMap || {});
  container.querySelectorAll(".alert-card").forEach((card) => {
    card.addEventListener("click", (e) => {
      if (e.target.closest("[data-alert-detail], .ac-check, .alert-select, .alert-select-all-row, a.ac-phone, .alert-contacts"))
        return;
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
  const sel = typeof selectedAlertIds === "function" ? selectedAlertIds() : new Set();
  const canWrite = typeof canWriteAlerts === "function" && canWriteAlerts();
  return `<table class="data alert-table"><thead><tr>
    ${
      canWrite
        ? `<th class="col-check"><input type="checkbox" id="alert-select-all" title="本页全选" /></th>`
        : ""
    }
    <th class="col-st">状态</th>
    <th class="col-sev">级别</th>
    <th class="col-name">告警名称</th>
    <th class="col-ip">告警 IP</th>
    <th class="col-host">主机名</th>
    <th class="col-sum">告警描述</th>
    <th class="col-contact">联系人</th>
    <th class="col-src">来源</th>
    <th class="col-dur">持续</th>
    <th class="col-time">开始时间</th>
    <th class="col-time">末次发生</th>
    <th class="col-group">组</th>
    <th class="col-key">键</th>
    <th class="col-time">最后评估</th>
    <th class="col-ack">接手</th>
    <th class="col-mw">维护</th>
    <th class="col-note">备注</th>
    <th class="col-tally">次数</th>
    <th class="col-act"></th>
  </tr></thead><tbody>${rows
    .map((a, i) => {
      const name = alertDisplayName(a);
      const summary = alertSummary(a);
      const preview = alertSummaryPreview(a, 100);
      const dur = alertDuration(a);
      const src = alertSourceLabel(a, ingressMap);
      const ip = alertTargetIp(a);
      const host = alertHostname(a);
      const inst = alertInstance(a);
      const group = alertGroup(a);
      const key = alertKey(a);
      const note = alertNoteHint(a);
      const tally = alertTally(a);
      const checked = sel.has(a.id);
      const instChip = inst
        ? `<div class="table-sub mono" title="实例 / 端口">实例：${esc(inst)}</div>`
        : "";
      const ackCell = alertIsAcked(a)
        ? `<span class="badge ack" title="${esc(a.ack_comment || "")}">${esc(
            a.assignee || a.acknowledged_by || "已确认"
          )}</span>`
        : `<span class="hint">—</span>`;
      const mwCell =
        [mwBadgeHtml(a), escalatedBadgeHtml(a)].filter(Boolean).join(" ") ||
        `<span class="hint">—</span>`;
      const noteCell = note
        ? `<span class="badge note" title="${esc(note)}">有</span>`
        : `<span class="hint">—</span>`;
      return `<tr data-alert-idx="${i}" class="${alertIsAcked(a) ? "is-acked" : ""}${
        checked ? " is-selected" : ""
      }">
    ${
      canWrite
        ? `<td class="col-check"><input type="checkbox" class="alert-select" data-alert-id="${esc(
            a.id
          )}" ${checked ? "checked" : ""} /></td>`
        : ""
    }
    <td class="col-st"><span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span></td>
    <td class="col-sev"><span class="sev sev-${esc(a.severity)}">${esc(severityLabel(a.severity))}</span></td>
    <td class="col-name">
      <div class="alert-name" title="${esc(name)}">${esc(name)}</div>
      ${instChip}
      <div class="alert-labels">${labelChips(
        a.labels,
        [
          "alertname",
          "severity",
          "source",
          "alertIp",
          "ip",
          "ipaddr",
          "instance",
          "host",
          "hostname",
          "主机名",
          "alertgroup",
          "AlertGroup",
          "alert_group",
          "alertkey",
          "AlertKey",
          "alert_key",
        ],
        3
      )}</div>
    </td>
    <td class="col-ip mono">${ip ? esc(ip) : '<span class="hint">—</span>'}</td>
    <td class="col-host" title="${esc(host)}">${host ? esc(host) : '<span class="hint">—</span>'}</td>
    <td class="col-sum" title="${esc(summary)}"><div class="alert-summary-text">${esc(
      preview || '<span class="hint">—</span>'
    )}</div></td>
    <td class="col-contact">${alertContactsHtml(a)}</td>
    <td class="col-src"><span class="source-tag" title="${esc(src)}">${esc(src)}</span></td>
    <td class="col-dur">${esc(dur)}</td>
    <td class="col-time">${esc(fmtTime(a.starts_at))}</td>
    <td class="col-time">${esc(fmtTime(a.last_occurrence_at || a.starts_at))}</td>
    <td class="col-group" title="${esc(group)}">${esc(group || "—")}</td>
    <td class="col-key mono" title="${esc(key)}">${esc(key || "—")}</td>
    <td class="col-time">${esc(fmtTime(a.last_evaluated_at))}</td>
    <td class="col-ack">${ackCell}</td>
    <td class="col-mw">${mwCell}</td>
    <td class="col-note">${noteCell}</td>
    <td class="col-tally mono">${tally}</td>
    <td class="col-act"><button type="button" data-alert-detail="${i}">详情</button></td>
  </tr>`;
    })
    .join("")}</tbody></table>`;
}

function bindAlertTableGrid(container, rows, ingressMap, reload) {
  if (!container || !rows?.length) return;
  bindAlertSelection(container, rows, reload);
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
      const preview = alertSummaryPreview(a, 80);
      const src = alertSourceLabel(a, ingressMap);
      return `<article class="alert-card compact status-${esc(a.status)}" data-alert-idx="${i}" tabindex="0" role="button">
        <div class="ac-rail"></div>
        <div class="ac-body">
          <div class="ac-top">
            <div class="ac-title-row">
              <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
              ${a.acknowledged_at ? `<span class="badge ack">已接手</span>` : ""}
              <h3 class="ac-name">${esc(name)}</h3>
            </div>
            <span class="ac-when">${esc(fmtTime(a.last_evaluated_at))}</span>
          </div>
          <p class="ac-summary" title="${esc(summary || src)}">${esc(preview || src)}</p>
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

/** List-friendly one-line preview; full text stays on title / detail. */
function alertSummaryPreview(a, maxLen = 120) {
  const raw = String(alertSummary(a) || "")
    .replace(/\s+/g, " ")
    .trim();
  if (!raw) return "";
  if (raw.length <= maxLen) return raw;
  return `${raw.slice(0, maxLen)}…`;
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
             : n.transition === "escalated"
             ? "升级通知"
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
        ${ackBadgeHtml(a) ? `<span style="margin-left:8px">${ackBadgeHtml(a)}</span>` : ""}
        ${mwBadgeHtml(a) ? `<span style="margin-left:8px">${mwBadgeHtml(a)}</span>` : ""}
        ${
          escalatedBadgeHtml(a)
            ? `<span style="margin-left:8px">${escalatedBadgeHtml(a)}</span>`
            : ""
        }
        <span class="source-tag" style="margin-left:8px">${esc(srcLabel)}</span>
        ${ip ? `<span class="ac-ip" style="margin-left:8px">IP ${esc(ip)}</span>` : ""}
      </p>
    </div>
    <div class="modal-body alert-detail">
      <div class="field alert-contact-box">
        <label>联系人 / 手机号</label>
        ${
          alertContacts(a).length
            ? alertContactsHtml(a)
            : `<div class="hint">暂无。请在「告警丰富」台账中配置联系人、手机号（多人可用逗号或顿号分隔）。</div>`
        }
      </div>
      <div class="detail-grid">
        <div><label>主机名</label><div>${esc(alertHostname(a) || "—")}</div></div>
        <div><label>告警 IP</label><div class="mono">${esc(ip || "—")}</div></div>
        <div><label>AlertGroup</label><div>${esc(alertGroup(a) || "—")}</div></div>
        <div><label>AlertKey</label><div class="mono">${esc(alertKey(a) || "—")}</div></div>
        <div><label>次数 (Tally)</label><div class="mono">${alertTally(a)}</div></div>
        <div><label>当前值</label><div class="mono">${fmtValue(a.value)}</div></div>
        <div><label>持续时长</label><div>${esc(alertDuration(a))}</div></div>
        <div><label>开始时间</label><div>${esc(fmtTime(a.starts_at))}</div></div>
        <div><label>末次发生</label><div>${esc(
          fmtTime(a.last_occurrence_at || a.starts_at)
        )}</div></div>
        <div><label>结束时间</label><div>${esc(a.ends_at ? fmtTime(a.ends_at) : "—")}</div></div>
        <div><label>最后评估</label><div>${esc(fmtTime(a.last_evaluated_at))}</div></div>
        <div><label>Pending 起</label><div>${esc(
          a.pending_since ? fmtTime(a.pending_since) : "—"
        )}</div></div>
        <div><label>已通知触发</label><div>${a.notified_firing ? "是" : "否"}</div></div>
        <div><label>已通知恢复</label><div>${a.notified_resolved ? "是" : "否"}</div></div>
        <div><label>接手人</label><div>${esc(
          alertIsAcked(a) ? a.assignee || a.acknowledged_by || "—" : "未接手"
        )}</div></div>
        <div><label>确认时间</label><div>${esc(
          a.acknowledged_at ? fmtTime(a.acknowledged_at) : "—"
        )}</div></div>
        <div><label>接手备注</label><div>${esc(a.ack_comment || "—")}</div></div>
        <div><label>升级时间</label><div>${esc(
          a.escalated_at ? fmtTime(a.escalated_at) : "—"
        )}</div></div>
        <div><label>关闭人</label><div>${esc(a.closed_by || "—")}</div></div>
        <div><label>关闭时间</label><div>${esc(
          a.closed_at ? fmtTime(a.closed_at) : "—"
        )}</div></div>
        <div><label>关闭原因</label><div>${esc(a.close_comment || "—")}</div></div>
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
      ${
        canWriteAlerts()
          ? `${
              a.status !== "resolved"
                ? `<button type="button" class="primary" id="m-close-alert">关闭告警</button>`
                : ""
            }
             ${
               alertIsAcked(a)
                 ? `<button type="button" class="ghost" id="m-unack">取消接手</button>`
                 : a.status !== "resolved"
                 ? `<button type="button" class="ghost" id="m-ack">确认接手</button>`
                 : ""
             }`
          : ""
      }
      <button type="button" class="ghost" id="m-silence">据此静默</button>
      <button type="button" class="ghost" id="m-mw">据此开维护</button>
      <button type="button" class="ghost" id="m-close">关闭</button>
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
  document.getElementById("m-mw").onclick = () => {
    closeModal();
    maintenanceFromAlert(a);
  };
  const ackBtn = document.getElementById("m-ack");
  if (ackBtn) {
    ackBtn.onclick = () => {
      closeModal();
      openAckModal(a);
    };
  }
  const unackBtn = document.getElementById("m-unack");
  if (unackBtn) {
    unackBtn.onclick = async () => {
      try {
        await unackAlert(a);
        closeModal();
        toast("已取消接手");
        navigate("alerts");
      } catch (err) {
        toast(err.message || "操作失败", true);
      }
    };
  }
  const closeAlertBtn = document.getElementById("m-close-alert");
  if (closeAlertBtn) {
    closeAlertBtn.onclick = () => {
      closeModal();
      openCloseModal(a);
    };
  }
}


  async function renderAlerts(root) {
    const f = state.alertFilters || {
      status: "",
      severity: "",
      source: "",
      q: "",
      ip: "",
      store: "",
      acked: "",
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
      const acked =
        root.querySelector("#alert-acked")?.value ?? state.alertFilters?.acked ?? "";
      let page = Number(state.alertFilters?.page) || 1;
      if (page < 1) page = 1;
      const limit = ALERT_PAGE_SIZE;
      state.alertFilters = { status, severity, source, q, ip, store, acked, page };

      const params = new URLSearchParams();
      if (status) params.set("status", status);
      if (severity) params.set("severity", severity);
      if (source) params.set("source", source);
      if (q) params.set("q", q);
      if (ip) params.set("ip", ip);
      if (store) params.set("store", store);
      if (acked) params.set("acked", acked);
      params.set("page", String(page));
      params.set("limit", String(limit));

      const [data, ingressRows, mwRows] = await Promise.all([
        api("/api/alerts?" + params.toString()),
        api("/api/ingress").catch(() => []),
        api("/api/maintenance-windows").catch(() => []),
      ]);
      state.cache = state.cache || {};
      state.cache.maintenanceWindows = mwRows || [];
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
      pruneAlertSelection(rows);
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
            <select id="alert-acked" title="接手状态">
              <option value="" ${!acked ? "selected" : ""}>接手·全部</option>
              <option value="false" ${acked === "false" ? "selected" : ""}>未接手</option>
              <option value="true" ${acked === "true" ? "selected" : ""}>已接手</option>
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
        <div data-alert-bulk-host>${alertBulkBarHtml(canWriteAlerts())}</div>
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

      const reload = () => load();
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
      if (view === "table") bindAlertTableGrid(root, rows, ingressMap, reload);
      else bindAlertCards(root, rows, ingressMap, reload);
      bindAlertBulkBar(root, rows, reload);
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
      root.querySelector("#alert-acked").onchange = (e) => {
        state.alertFilters = {
          ...(state.alertFilters || {}),
          acked: e.target.value,
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
