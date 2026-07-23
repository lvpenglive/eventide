/* Eventide console SPA */
(() => {
  const TOKEN_KEY = "eventide_token";
  const USER_KEY = "eventide_user";
  const SIDEBAR_KEY = "eventide_sidebar_collapsed";

  const state = {
    page: "overview",
    cache: {},
    me: null,
  };

  const titles = {
    overview: ["总览", "系统运行状态与近期告警"],
    datasources: ["数据源", "Prometheus / VictoriaMetrics"],
    rules: ["告警规则", "阈值评估与通知绑定"],
    channels: ["通知渠道", "Webhook · 钉钉 · 企微 · 飞书"],
    ingress: ["Ingress 接入", "外部告警接入 · 试推送 · 通知绑定"],
    alerts: ["告警事件", "按状态浏览 · 点开看详情"],
    silences: ["静默策略", "按规则或标签临时抑制通知"],
    settings: ["系统设置", "运行配置与账号信息（只读）"],
  };

  // ---------- API ----------
  function token() {
    return localStorage.getItem(TOKEN_KEY);
  }

  async function api(path, opts = {}) {
    const headers = Object.assign({}, opts.headers || {});
    if (!(opts.body instanceof FormData) && opts.body && !headers["Content-Type"]) {
      headers["Content-Type"] = "application/json";
    }
    const t = token();
    if (t) headers.Authorization = `Bearer ${t}`;
    const res = await fetch(path, { ...opts, headers });
    if (res.status === 401 && !path.includes("/api/auth/login")) {
      logout(false);
      throw new Error("登录已失效");
    }
    if (res.status === 204) return null;
    const text = await res.text();
    let data = null;
    try {
      data = text ? JSON.parse(text) : null;
    } catch {
      data = { error: text };
    }
    if (!res.ok) throw new Error((data && data.error) || text || res.statusText);
    return data;
  }

  function toast(msg, err = false) {
    const el = document.getElementById("toast");
    el.textContent = msg;
    el.classList.toggle("err", !!err);
    el.classList.add("show");
    clearTimeout(toast._t);
    toast._t = setTimeout(() => el.classList.remove("show"), 2800);
  }

  function esc(s) {
    return String(s ?? "").replace(/[&<>"']/g, (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c])
    );
  }

  function cmpLabel(c) {
    return { gt: ">", gte: ">=", lt: "<", lte: "<=", eq: "==", neq: "!=" }[c] || c;
  }

  function fmtTime(s) {
    if (!s) return "—";
    try {
      return new Date(s).toLocaleString();
    } catch {
      return s;
    }
  }

  // ---------- Auth views ----------
  function showLogin() {
    document.getElementById("view-login").style.display = "grid";
    document.getElementById("view-app").classList.remove("active");
  }

  function showApp() {
    document.getElementById("view-login").style.display = "none";
    document.getElementById("view-app").classList.add("active");
    const name = localStorage.getItem(USER_KEY) || "admin";
    document.getElementById("user-name").textContent = name;
    const av = document.getElementById("user-avatar");
    if (av) {
      av.textContent = (name[0] || "A").toUpperCase();
      av.title = name;
    }
    applySidebarState();
  }

  function applySidebarState() {
    const app = document.getElementById("view-app");
    const collapsed = localStorage.getItem(SIDEBAR_KEY) === "1";
    app.classList.toggle("sidebar-collapsed", collapsed);
    const btn = document.getElementById("btn-sidebar");
    if (btn) btn.title = collapsed ? "展开侧栏" : "收起侧栏";
  }

  function toggleSidebar() {
    const next = localStorage.getItem(SIDEBAR_KEY) === "1" ? "0" : "1";
    localStorage.setItem(SIDEBAR_KEY, next);
    applySidebarState();
  }

  function logout(notify = true) {
    localStorage.removeItem(TOKEN_KEY);
    localStorage.removeItem(USER_KEY);
    showLogin();
    if (notify) toast("已退出登录");
  }

  async function tryBoot() {
    if (!token()) {
      showLogin();
      return;
    }
    try {
      state.me = await api("/api/auth/me");
      localStorage.setItem(USER_KEY, state.me.username);
      showApp();
      navigate(state.page);
    } catch {
      showLogin();
    }
  }

  document.getElementById("login-form").addEventListener("submit", async (e) => {
    e.preventDefault();
    const err = document.getElementById("login-error");
    err.textContent = "";
    const username = document.getElementById("username").value.trim();
    const password = document.getElementById("password").value;
    try {
      const data = await api("/api/auth/login", {
        method: "POST",
        body: JSON.stringify({ username, password }),
      });
      localStorage.setItem(TOKEN_KEY, data.token);
      localStorage.setItem(USER_KEY, data.username);
      state.me = await api("/api/auth/me");
      showApp();
      navigate("overview");
      toast("登录成功");
    } catch (ex) {
      err.textContent = ex.message || "登录失败";
    }
  });

  document.getElementById("btn-logout").addEventListener("click", () => logout());
  document.getElementById("btn-sidebar").addEventListener("click", () => toggleSidebar());

  // ---------- Navigation ----------
  document.querySelectorAll(".nav-item").forEach((btn) => {
    btn.addEventListener("click", () => navigate(btn.dataset.page));
  });

  function navigate(page) {
    state.page = page;
    document.querySelectorAll(".nav-item").forEach((b) => {
      b.classList.toggle("active", b.dataset.page === page);
    });
    const [t, s] = titles[page] || [page, ""];
    document.getElementById("page-title").textContent = t;
    document.getElementById("page-sub").textContent = s;
    document.getElementById("page-actions").innerHTML = "";
    renderPage();
  }

  async function renderPage() {
    const root = document.getElementById("page-root");
    root.innerHTML = `<div class="panel empty">加载中…</div>`;
    try {
      const fn = pages[state.page];
      await fn(root);
    } catch (e) {
      root.innerHTML = `<div class="panel empty">加载失败：${esc(e.message)}</div>`;
    }
  }

  function setActions(html) {
    document.getElementById("page-actions").innerHTML = html;
  }

  // ---------- Modal ----------
  const modal = document.getElementById("modal");
  const modalBody = document.getElementById("modal-body");

  function openModal(html) {
    modalBody.innerHTML = html;
    modal.classList.add("open");
  }
  function closeModal() {
    modal.classList.remove("open");
    modalBody.innerHTML = "";
  }
  modal.addEventListener("click", (e) => {
    if (e.target === modal) closeModal();
  });

  function multiSelect(name, options, selected = []) {
    const set = new Set(selected);
    return `<select name="${name}" multiple size="${Math.min(6, Math.max(3, options.length || 3))}">
      ${options
        .map(
          (o) =>
            `<option value="${esc(o.value)}" ${set.has(o.value) ? "selected" : ""}>${esc(
              o.label
            )}</option>`
        )
        .join("")}
    </select>
    <div style="font-size:0.75rem;color:var(--muted);margin-top:0.25rem">按住 Ctrl/⌘ 多选</div>`;
  }

  function selectedValues(form, name) {
    const el = form.elements.namedItem(name);
    if (!el) return [];
    return [...el.selectedOptions].map((o) => o.value);
  }

  // ---------- Pages ----------
  const pages = {
    async overview(root) {
      setActions(`<button class="ghost" id="btn-refresh">刷新</button>`);
      document.getElementById("btn-refresh").onclick = () => renderPage();
      const d = await api("/api/overview");
      const ingressRows = await api("/api/ingress").catch(() => []);
      const ingressMap = Object.fromEntries((ingressRows || []).map((r) => [r.id, r]));
      root.innerHTML = `
        <div class="stats">
          <div class="stat firing"><div class="n">${d.alerts_firing}</div><div class="l">正在告警</div></div>
          <div class="stat"><div class="n">${d.alerts_pending}</div><div class="l">等待中</div></div>
          <div class="stat ok"><div class="n">${d.alerts_resolved}</div><div class="l">已恢复</div></div>
          <div class="stat accent"><div class="n">${d.enabled_rules}/${d.rules}</div><div class="l">启用规则</div></div>
          <div class="stat"><div class="n">${d.datasources}</div><div class="l">数据源</div></div>
          <div class="stat"><div class="n">${d.channels}</div><div class="l">通知渠道</div></div>
          <div class="stat"><div class="n">${d.ingress_routes}</div><div class="l">Ingress</div></div>
          <div class="stat"><div class="n">${d.active_silences}</div><div class="l">生效静默</div></div>
        </div>
        <div class="panel">
          <h3 style="margin:0 0 0.75rem;font-size:1rem">最近告警</h3>
          ${alertTable(d.recent_alerts || [], { compact: true, ingressMap })}
        </div>`;
      bindAlertTable(root, d.recent_alerts || [], ingressMap);
    },

    async datasources(root) {
      setActions(`<button class="primary" id="btn-add">新建数据源</button>`);
      document.getElementById("btn-add").onclick = () => editDatasource();
      const rows = await api("/api/datasources");
      state.cache.datasources = rows;
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>名称</th><th>类型</th><th>URL</th><th>状态</th><th></th></tr></thead>
            <tbody>${rows
              .map(
                (d) => `<tr>
              <td>${esc(d.name)}</td>
              <td>${esc(d.kind)}</td>
              <td class="mono">${esc(d.url)}</td>
              <td><span class="badge ${d.enabled ? "on" : "off"}">${d.enabled ? "启用" : "停用"}</span></td>
              <td class="actions">
                <button data-edit="${d.id}">编辑</button>
                <button class="danger" data-del="${d.id}">删除</button>
              </td></tr>`
              )
              .join("")}</tbody></table>`
          : `<div class="empty">还没有数据源。支持 prometheus / victoriametrics / kafka / log。</div>`
      }</div>`;
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editDatasource(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该数据源？")) return;
          await api(`/api/datasources/${b.dataset.del}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
    },

    async rules(root) {
      setActions(`<button class="primary" id="btn-add">新建规则</button>`);
      document.getElementById("btn-add").onclick = () => editRule();
      const [rows, dss, chs] = await Promise.all([
        api("/api/rules"),
        api("/api/datasources"),
        api("/api/channels"),
      ]);
      state.cache.datasources = dss;
      state.cache.channels = chs;
      const dsName = Object.fromEntries(dss.map((d) => [d.id, d.name]));
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>名称</th><th>数据源</th><th>表达式</th><th>条件</th><th>间隔</th><th>状态</th><th></th></tr></thead>
            <tbody>${rows
              .map(
                (r) => `<tr>
              <td>${esc(r.name)}</td>
              <td>${esc(dsName[r.datasource_id] || r.datasource_id.slice(0, 8))}</td>
              <td class="mono">${esc(r.expr)}</td>
              <td class="mono">${esc(cmpLabel(r.comparator))} ${r.threshold}</td>
              <td>${r.interval_seconds}s / for ${r.for_seconds}s</td>
              <td><span class="badge ${r.enabled ? "on" : "off"}">${r.enabled ? "启用" : "停用"}</span></td>
              <td class="actions">
                <button data-run="${r.id}">试跑</button>
                <button data-edit="${r.id}">编辑</button>
                <button class="danger" data-del="${r.id}">删除</button>
              </td></tr>`
              )
              .join("")}</tbody></table>`
          : `<div class="empty">还没有规则。配置数据源后即可创建阈值告警。</div>`
      }</div>`;
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editRule(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该规则？")) return;
          await api(`/api/rules/${b.dataset.del}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
      root.querySelectorAll("[data-run]").forEach((b) => {
        b.onclick = async () => {
          try {
            await api(`/api/rules/${b.dataset.run}/evaluate`, { method: "POST" });
            toast("试跑完成");
          } catch (e) {
            toast(e.message, true);
          }
        };
      });
    },

    async channels(root) {
      setActions(`<button class="primary" id="btn-add">新建渠道</button>`);
      document.getElementById("btn-add").onclick = () => editChannel();
      const rows = await api("/api/channels");
      state.cache.channels = rows;
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>名称</th><th>类型</th><th>URL</th><th>状态</th><th></th></tr></thead>
            <tbody>${rows
              .map(
                (c) => `<tr>
              <td>${esc(c.name)}</td>
              <td>${esc(c.kind)}</td>
              <td class="mono">${esc(c.url)}</td>
              <td><span class="badge ${c.enabled ? "on" : "off"}">${c.enabled ? "启用" : "停用"}</span></td>
              <td class="actions">
                <button data-edit="${c.id}">编辑</button>
                <button class="danger" data-del="${c.id}">删除</button>
              </td></tr>`
              )
              .join("")}</tbody></table>`
          : `<div class="empty">还没有通知渠道。支持 webhook / dingtalk / wecom / feishu。</div>`
      }</div>`;
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editChannel(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该渠道？")) return;
          await api(`/api/channels/${b.dataset.del}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
    },

    async ingress(root) {
      setActions(`<button class="primary" id="btn-add">新建 Ingress</button>`);
      document.getElementById("btn-add").onclick = () => editIngress();
      const [rows, chs] = await Promise.all([api("/api/ingress"), api("/api/channels")]);
      state.cache.channels = chs;
      const chMap = Object.fromEntries(chs.map((c) => [c.id, c]));
      const origin = location.origin;

      if (!rows.length) {
        root.innerHTML = `
          <div class="panel guide">
            <h3>从 Ingress 接入告警</h3>
            <ol class="steps">
              <li>先在「通知渠道」配置至少一个机器人 / Webhook（可选，也可稍后绑定）</li>
              <li>创建 Ingress：选择 Alertmanager / Generic（含拨测） / Kafka</li>
              <li>把外部平台 Webhook 指到下方生成的地址，或使用「试推送」验证</li>
              <li>在「告警事件」查看 firing / resolved 与通知结果</li>
            </ol>
            <button class="primary" id="btn-guide-add">新建第一个 Ingress</button>
          </div>`;
        document.getElementById("btn-guide-add").onclick = () => editIngress();
        return;
      }

      root.innerHTML = `<div class="ingress-list">${rows
        .map((r) => {
          const url =
            r.kind === "kafka"
              ? `kafka://${r.endpoint || ""}/${(r.options && r.options.topic) || ""}`
              : r.kind === "alertmanager"
              ? `${origin}/api/ingress/${r.id}/alertmanager`
              : `${origin}/api/ingress/${r.id}/generic`;
          const pushUrl = `${origin}/api/ingress/${r.id}/push`;
          const chNames = (r.channel_ids || [])
            .map((id) => (chMap[id] ? chMap[id].name : id.slice(0, 8)))
            .join("、") || "未绑定渠道";
          const kindHint =
            r.kind === "alertmanager"
              ? "接收 Prometheus Alertmanager webhook"
              : r.kind === "kafka"
              ? "后台消费 Topic 中的告警 JSON"
              : "通用 JSON / Jeecg 拨测 probe-alert";
          return `<article class="ingress-card">
            <div class="ic-head">
              <div>
                <div class="ic-title">${esc(r.name)}</div>
                <div class="ic-sub"><span class="badge ${r.enabled ? "on" : "off"}">${
                  r.enabled ? "启用" : "停用"
                }</span> <span class="kind-tag">${esc(r.kind)}</span> · ${esc(kindHint)}</div>
              </div>
              <div class="actions">
                <button data-test="${r.id}" ${r.enabled ? "" : "disabled"}>试推送</button>
                <button data-alerts="${r.id}">查告警</button>
                <button data-edit="${r.id}">编辑</button>
                <button class="danger" data-del="${r.id}">删除</button>
              </div>
            </div>
            <div class="ic-body">
              <div class="field">
                <label>接入地址</label>
                <div class="url-row">
                  <code class="mono url-box">${esc(url)}</code>
                  <button data-copy="${esc(url)}">复制</button>
                </div>
                ${
                  r.kind !== "kafka"
                    ? `<div class="hint">也可用自动识别入口：<code class="mono">${esc(
                        pushUrl
                      )}</code>
                    ${
                      r.token
                        ? `· 请求头需带 <code>Authorization: Bearer ***</code> 或 <code>X-Eventide-Token</code>`
                        : "· 未配置 Token，任意来源可推送"
                    }</div>`
                    : `<div class="hint">Brokers <code class="mono">${esc(
                        r.endpoint || "—"
                      )}</code> · Topic <code class="mono">${esc(
                        (r.options && r.options.topic) || "—"
                      )}</code> · 起始 ${(r.options && r.options.start) || "latest"}</div>`
                }
              </div>
              <div class="field" style="margin:0">
                <label>通知渠道</label>
                <div>${esc(chNames)}</div>
              </div>
            </div>
          </article>`;
        })
        .join("")}</div>`;

      root.querySelectorAll("[data-copy]").forEach((b) => {
        b.onclick = async () => {
          await navigator.clipboard.writeText(b.dataset.copy);
          toast("已复制");
        };
      });
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editIngress(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该 Ingress？")) return;
          await api(`/api/ingress/${b.dataset.del}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
      root.querySelectorAll("[data-alerts]").forEach((b) => {
        b.onclick = () => {
          state.alertFilters = { source: "ingress", q: "" };
          navigate("alerts");
        };
      });
      root.querySelectorAll("[data-test]").forEach((b) => {
        b.onclick = () => openIngressTest(rows.find((x) => x.id === b.dataset.test));
      });
    },

    async alerts(root) {
      const f = state.alertFilters || { status: "", severity: "", source: "", q: "" };
      setActions(`<button class="ghost" id="btn-refresh">刷新</button>`);

      const load = async () => {
        const status = state.alertFilters?.status || "";
        const severity = root.querySelector("#alert-sev")?.value ?? state.alertFilters?.severity ?? "";
        const source = root.querySelector("#alert-source")?.value ?? state.alertFilters?.source ?? "";
        const q = (root.querySelector("#alert-q")?.value ?? state.alertFilters?.q ?? "").trim();
        state.alertFilters = { status, severity, source, q };

        const params = new URLSearchParams();
        if (status) params.set("status", status);
        if (severity) params.set("severity", severity);
        if (source) params.set("source", source);
        if (q) params.set("q", q);

        const [rows, allRows, ingressRows] = await Promise.all([
          api("/api/alerts" + (params.toString() ? `?${params}` : "")),
          api("/api/alerts").catch(() => []),
          api("/api/ingress").catch(() => []),
        ]);
        const ingressMap = Object.fromEntries((ingressRows || []).map((r) => [r.id, r]));
        const nFire = allRows.filter((a) => a.status === "firing").length;
        const nPend = allRows.filter((a) => a.status === "pending").length;
        const nRes = allRows.filter((a) => a.status === "resolved").length;

        root.innerHTML = `
          <div class="alert-toolbar">
            <div class="alert-tabs" role="tablist">
              <button type="button" class="alert-tab ${!status ? "on" : ""}" data-st="">全部</button>
              <button type="button" class="alert-tab firing ${status === "firing" ? "on" : ""}" data-st="firing">告警中 <em>${nFire}</em></button>
              <button type="button" class="alert-tab ${status === "pending" ? "on" : ""}" data-st="pending">等待 <em>${nPend}</em></button>
              <button type="button" class="alert-tab ok ${status === "resolved" ? "on" : ""}" data-st="resolved">已恢复 <em>${nRes}</em></button>
            </div>
            <div class="alert-filters">
              <input id="alert-q" placeholder="搜索名称、摘要…" value="${esc(q)}" />
              <select id="alert-sev">
                <option value="">级别</option>
                <option value="critical" ${severity === "critical" ? "selected" : ""}>critical</option>
                <option value="warning" ${severity === "warning" ? "selected" : ""}>warning</option>
                <option value="info" ${severity === "info" ? "selected" : ""}>info</option>
              </select>
              <select id="alert-source">
                <option value="">来源</option>
                <option value="ingress" ${source === "ingress" ? "selected" : ""}>Ingress</option>
                <option value="rule" ${source === "rule" ? "selected" : ""}>规则</option>
              </select>
            </div>
          </div>
          ${
            rows.length
              ? alertCards(rows, ingressMap)
              : `<div class="panel empty">
                  暂无告警。去 Ingress「试推送」或等待规则触发。
                  <div style="margin-top:12px"><button class="primary" id="go-ingress">去 Ingress</button></div>
                </div>`
          }`;

        root.querySelectorAll(".alert-tab").forEach((tab) => {
          tab.onclick = () => {
            state.alertFilters = {
              ...(state.alertFilters || {}),
              status: tab.dataset.st || "",
            };
            load();
          };
        });
        bindAlertCards(root, rows, ingressMap);
        const go = document.getElementById("go-ingress");
        if (go) go.onclick = () => navigate("ingress");
        let t;
        const qEl = root.querySelector("#alert-q");
        qEl.oninput = () => {
          clearTimeout(t);
          t = setTimeout(() => {
            state.alertFilters = { ...(state.alertFilters || {}), q: qEl.value.trim() };
            load();
          }, 280);
        };
        root.querySelector("#alert-sev").onchange = (e) => {
          state.alertFilters = { ...(state.alertFilters || {}), severity: e.target.value };
          load();
        };
        root.querySelector("#alert-source").onchange = (e) => {
          state.alertFilters = { ...(state.alertFilters || {}), source: e.target.value };
          load();
        };
      };

      document.getElementById("btn-refresh").onclick = () => load();
      // seed filters from initial f
      state.alertFilters = { ...f };
      await load();
    },

    async silences(root) {
      setActions(`<button class="primary" id="btn-add">新建静默</button>`);
      document.getElementById("btn-add").onclick = () => editSilence();
      const rows = await api("/api/silences");
      const now = Date.now();
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>注释</th><th>规则</th><th>匹配标签</th><th>时间窗</th><th>状态</th><th></th></tr></thead>
            <tbody>${rows
              .map((s) => {
                const active = new Date(s.starts_at) <= now && now < new Date(s.ends_at);
                return `<tr>
              <td>${esc(s.comment || "—")}</td>
              <td class="mono">${s.rule_id ? esc(s.rule_id.slice(0, 8)) + "…" : "全部"}</td>
              <td class="mono">${esc(JSON.stringify(s.matchers || {}))}</td>
              <td>${esc(fmtTime(s.starts_at))} → ${esc(fmtTime(s.ends_at))}</td>
              <td><span class="badge ${active ? "on" : "off"}">${active ? "生效中" : "未生效"}</span></td>
              <td class="actions"><button class="danger" data-del="${s.id}">删除</button></td>
            </tr>`;
              })
              .join("")}</tbody></table>`
          : `<div class="empty">暂无静默策略。</div>`
      }</div>`;
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该静默？")) return;
          await api(`/api/silences/${b.dataset.del}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
    },

    async settings(root) {
      const me = await api("/api/auth/me");
      state.me = me;
      root.innerHTML = `
        <div class="panel" style="max-width:640px">
          <h3 style="margin:0 0 1rem;font-size:1rem">运行信息</h3>
          <div class="field"><label>当前用户</label><div>${esc(me.username)}</div></div>
          <div class="field"><label>监听地址</label><div class="mono">${esc(me.listen)}</div></div>
          <div class="field"><label>数据库</label><div class="mono">${esc(me.database_path)}</div></div>
          <div class="field"><label>静态资源目录</label><div class="mono">${esc(me.static_dir)}</div></div>
          <div class="field"><label>调度周期</label><div>${me.scheduler_tick_seconds} 秒</div></div>
          <div class="field"><label>Token 有效期</label><div>${me.token_ttl_hours} 小时</div></div>
          <p style="color:var(--muted);font-size:0.88rem;margin:1rem 0 0">
            账号密码与 JWT 密钥请修改 <code>eventide.toml</code> 中的 <code>[auth]</code> 段后重启服务。
          </p>
        </div>`;
    },
  };

  function alertCards(rows, ingressMap) {
    return `<div class="alert-feed">${rows
      .map((a, i) => {
        const name = alertDisplayName(a);
        const summary = alertSummary(a);
        const dur = alertDuration(a);
        const src = alertSourceLabel(a, ingressMap);
        const val = fmtValue(a.value);
        const chips = labelChips(a.labels, ["alertname", "severity", "source"], 3);
        return `<article class="alert-card status-${esc(a.status)} sev-${esc(
          a.severity
        )}" data-alert-idx="${i}" tabindex="0" role="button">
          <div class="ac-rail" aria-hidden="true"></div>
          <div class="ac-body">
            <div class="ac-top">
              <div class="ac-title-row">
                <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
                <span class="sev sev-${esc(a.severity)}">${esc(a.severity)}</span>
                <h3 class="ac-name">${esc(name)}</h3>
              </div>
              <button type="button" class="ghost ac-detail" data-alert-detail="${i}">详情</button>
            </div>
            <p class="ac-summary">${esc(summary || "无摘要")}</p>
            <div class="ac-meta">
              <span>${esc(src)}</span>
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
    });
    container.querySelectorAll("[data-alert-detail]").forEach((btn) => {
      btn.addEventListener("click", (e) => {
        e.stopPropagation();
        open(Number(btn.dataset.alertDetail));
      });
    });
  }

  function alertTable(rows, opts = {}) {
    if (!rows.length) return `<div class="empty">暂无告警事件</div>`;
    const compact = !!opts.compact;
    const ingressMap = opts.ingressMap || {};
    if (!compact) {
      return alertCards(rows, ingressMap);
    }
    // Overview: compact rows
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
      if (name) return `Ingress · ${name}`;
      return kind === "alertmanager" ? "AM 接入" : kind === "kafka" ? "Kafka 接入" : "Ingress";
    }
    return "规则评估";
  }

  function alertDisplayName(a) {
    const l = a.labels || {};
    return l.alertname || l.bizchainName || l.businessName || l.rule || "未命名告警";
  }

  function alertSummary(a) {
    const an = a.annotations || {};
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

  async function showAlertDetail(a, ingressMap = {}) {
    if (!a) return;
    const name = alertDisplayName(a);
    const srcLabel = alertSourceLabel(a, ingressMap);
    let notifies = [];
    try {
      notifies = await api(`/api/alerts/${a.id}/notifies`);
    } catch {
      notifies = [];
    }
    const notifyHtml = notifies.length
      ? `<table class="data"><thead><tr><th>时间</th><th>边沿</th><th>结果</th><th>错误</th></tr></thead>
         <tbody>${notifies
           .map(
             (n) => `<tr>
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
           <td class="mono">${esc(n.error || "—")}</td>
         </tr>`
           )
           .join("")}</tbody></table>`
      : `<div class="empty" style="padding:8px 0">暂无通知记录（可能未绑定渠道，或尚未发生状态边沿）</div>`;

    openModal(`
      <div class="modal-head">
        <h3>${esc(name)}</h3>
        <p class="desc">
          <span class="badge ${esc(a.status)}">${esc(statusLabel(a.status))}</span>
          <span class="sev sev-${esc(a.severity)}" style="margin-left:8px">${esc(a.severity)}</span>
          <span class="source-tag" style="margin-left:8px">${esc(srcLabel)}</span>
        </p>
      </div>
      <div class="modal-body alert-detail">
        <div class="detail-grid">
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
          <label>摘要 / 注解</label>
          <div class="summary-box">${esc(alertSummary(a) || "—")}</div>
          ${kvTable(a.annotations)}
        </div>
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
      </div>`);
    document.getElementById("m-close").onclick = closeModal;
    document.getElementById("m-silence").onclick = () => {
      closeModal();
      const matchers = { ...(a.labels || {}) };
      delete matchers.source;
      editSilence({
        comment: `静默 ${alertDisplayName(a)}`,
        matchers,
        rule_id: null,
      });
    };
  }

  function openIngressTest(row) {
    if (!row) return;
    const isProbeFriendly = row.kind === "generic" || row.kind === "kafka";
    openModal(`
      <div class="modal-head">
        <h3>试推送 · ${esc(row.name)}</h3>
        <p class="desc">写入一条样例告警，走完整接入 → 去重 → 通知链路，用于验证配置。</p>
      </div>
      <div class="modal-body">
        <div class="field">
          <label>场景</label>
          <div class="type-list">
            <label class="type-row"><input type="radio" name="scenario" value="fire" checked />
              <span class="t-main"><span class="t-name">触发告警</span><span class="t-desc">Generic firing 样例</span></span></label>
            <label class="type-row"><input type="radio" name="scenario" value="recover" />
              <span class="t-main"><span class="t-name">恢复告警</span><span class="t-desc">同一告警标识的 resolved</span></span></label>
            ${
              isProbeFriendly
                ? `<label class="type-row"><input type="radio" name="scenario" value="probe_fire" />
              <span class="t-main"><span class="t-name">拨测失败</span><span class="t-desc">Jeecg probe-alert fire</span></span></label>
            <label class="type-row"><input type="radio" name="scenario" value="probe_recover" />
              <span class="t-main"><span class="t-name">拨测恢复</span><span class="t-desc">Jeecg probe-alert recover</span></span></label>`
                : ""
            }
          </div>
        </div>
        <div class="hint">试推送使用控制台登录态，不经过 Ingress Token 校验。</div>
      </div>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button type="button" class="primary" id="m-run">推送</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("m-run").onclick = async () => {
      const scenario =
        document.querySelector('input[name="scenario"]:checked')?.value || "fire";
      try {
        const res = await api(`/api/ingress/${row.id}/test`, {
          method: "POST",
          body: JSON.stringify({ scenario }),
        });
        closeModal();
        toast(res.hint || `已接受 ${res.accepted} 条`);
        state.alertFilters = { source: "ingress", status: "", severity: "", q: "" };
        navigate("alerts");
      } catch (e) {
        toast(e.message || "试推送失败", true);
      }
    };
  }

  // ---------- Editors ----------
  function editDatasource(row) {
    const kind = row?.kind || "prometheus";
    const opt = row?.options || {};
    const types = [
      { id: "prometheus", name: "Prometheus", desc: "PromQL 指标查询" },
      { id: "victoriametrics", name: "VictoriaMetrics", desc: "兼容 PromQL" },
      { id: "kafka", name: "Kafka", desc: "消息管道：解析 JSON 字段告警" },
      { id: "log", name: "Loki 日志", desc: "LogQL 日志统计" },
    ];
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑数据源" : "新建数据源"}</h3>
        <p class="desc">选择类型后填写连接信息，规则评估会按类型自动拉数。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field">
          <label>名称</label>
          <input name="name" required placeholder="例如：生产 Prometheus" value="${esc(row?.name || "")}" />
        </div>
        <div class="field">
          <label>类型</label>
          <div class="type-list">
            ${types
              .map(
                (t) => `<label class="type-row">
              <input type="radio" name="kind" value="${t.id}" ${kind === t.id ? "checked" : ""} />
              <span class="t-main">
                <span class="t-name">${t.name}</span>
                <span class="t-desc">${t.desc}</span>
              </span>
            </label>`
              )
              .join("")}
          </div>
        </div>
        <div class="field">
          <label class="check-row">
            <input type="checkbox" name="enabled" ${!row || row.enabled ? "checked" : ""} />
            <span>启用此数据源；停用后关联规则将跳过评估</span>
          </label>
        </div>

        <div class="kind-panel" data-kinds="prometheus,victoriametrics,log" id="panel-http">
          <div class="field">
            <label id="url-label">HTTP 地址</label>
            <input name="url_http" placeholder="http://127.0.0.1:9090" value="${esc(
              kind === "kafka" ? "" : row?.url || ""
            )}" />
            <div class="hint" id="url-hint">填写查询 API 根地址。</div>
          </div>
        </div>

        <div class="kind-panel" data-kinds="kafka" id="panel-kafka" hidden>
          <div class="field">
            <label>Brokers</label>
            <input name="url_kafka" placeholder="127.0.0.1:9092" value="${esc(
              kind === "kafka" ? row?.url || "" : ""
            )}" />
            <div class="hint">多个 broker 用英文逗号分隔。消息按 JSON 解析字段后走阈值规则。</div>
          </div>
          <div class="row">
            <div class="field">
              <label>Topic</label>
              <input name="topic" placeholder="orders" value="${esc(opt.topic || "")}" required />
            </div>
            <div class="field">
              <label>数值字段（JSON 路径）</label>
              <input name="field" placeholder="latency_ms 或 metrics.p99" value="${esc(opt.field || "")}" />
            </div>
          </div>
          <div class="row">
            <div class="field">
              <label>标签字段（可选）</label>
              <input name="label_fields" placeholder="service,instance" value="${esc(opt.label_fields || "")}" />
              <div class="hint">从消息中取出作为标签，用于分组与告警标识。</div>
            </div>
            <div class="field">
              <label>每次拉取条数</label>
              <input name="max_records" type="number" min="1" placeholder="100" value="${esc(opt.max_records || "100")}" />
            </div>
          </div>
          <div class="row">
            <div class="field">
              <label>评估模式</label>
              <select name="mode">
                <option value="field" ${!opt.mode || opt.mode === "field" ? "selected" : ""}>字段解析 field（推荐）</option>
                <option value="depth" ${opt.mode === "depth" ? "selected" : ""}>堆积深度 depth</option>
                <option value="count" ${opt.mode === "count" ? "selected" : ""}>近期消息数 count</option>
              </select>
            </div>
            <div class="field">
              <label>扫描分区数</label>
              <input name="partitions" type="number" min="1" placeholder="8" value="${esc(opt.partitions || "8")}" />
            </div>
          </div>
          <div class="hint">规则表达式可覆盖数值字段路径；例如消息 {"latency_ms":820,"service":"api"}，字段填 latency_ms，阈值 &gt; 500。</div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`);

    const form = document.getElementById("f");
    const syncKind = () => {
      const k = form.querySelector('input[name="kind"]:checked')?.value || "prometheus";
      form.querySelectorAll(".kind-panel").forEach((p) => {
        const kinds = (p.dataset.kinds || "").split(",");
        p.hidden = !kinds.includes(k);
      });
      const urlLabel = document.getElementById("url-label");
      const urlHint = document.getElementById("url-hint");
      const httpInput = form.querySelector('[name="url_http"]');
      if (k === "log") {
        urlLabel.textContent = "Loki 地址";
        httpInput.placeholder = "http://127.0.0.1:3100";
        urlHint.textContent = "填写 Loki 根地址。规则表达式使用 LogQL。";
      } else if (k === "victoriametrics") {
        urlLabel.textContent = "VictoriaMetrics 地址";
        httpInput.placeholder = "http://127.0.0.1:8428";
        urlHint.textContent = "兼容 PromQL 的查询入口。";
      } else {
        urlLabel.textContent = "Prometheus 地址";
        httpInput.placeholder = "http://127.0.0.1:9090";
        urlHint.textContent = "填写 Prometheus 查询 API 根地址。";
      }
    };
    form.querySelectorAll('input[name="kind"]').forEach((el) => {
      el.addEventListener("change", syncKind);
    });
    syncKind();

    document.getElementById("m-cancel").onclick = closeModal;
    form.onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(form);
      const k = String(fd.get("kind") || "prometheus");
      const options = {};
      let url = "";
      if (k === "kafka") {
        url = String(fd.get("url_kafka") || "").trim();
        const topic = String(fd.get("topic") || "").trim();
        if (!topic) {
          toast("请填写 Kafka Topic", true);
          return;
        }
        options.topic = topic;
        options.mode = String(fd.get("mode") || "field");
        const field = String(fd.get("field") || "").trim();
        if (field) options.field = field;
        const labelFields = String(fd.get("label_fields") || "").trim();
        if (labelFields) options.label_fields = labelFields;
        const maxRecords = String(fd.get("max_records") || "").trim();
        if (maxRecords) options.max_records = maxRecords;
        const partitions = String(fd.get("partitions") || "").trim();
        if (partitions) options.partitions = partitions;
      } else {
        url = String(fd.get("url_http") || "").trim();
      }
      if (!url) {
        toast("请填写连接地址", true);
        return;
      }
      const body = {
        name: fd.get("name"),
        kind: k,
        url,
        options,
        enabled: form.querySelector('[name="enabled"]').checked,
      };
      if (row) await api(`/api/datasources/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
      else await api("/api/datasources", { method: "POST", body: JSON.stringify(body) });
      closeModal();
      toast("已保存");
      renderPage();
    };
  }

  function editChannel(row) {
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑渠道" : "新建通知渠道"}</h3>
        <p class="desc">告警边沿触发时，向所选渠道发送通知。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>名称</label><input name="name" required placeholder="例如：值班钉钉群" value="${esc(
          row?.name || ""
        )}" /></div>
        <div class="field"><label>类型</label>
          <select name="kind">
            ${["webhook", "dingtalk", "wecom", "feishu"]
              .map(
                (k) =>
                  `<option value="${k}" ${row?.kind === k || (!row && k === "webhook") ? "selected" : ""}>${k}</option>`
              )
              .join("")}
          </select>
        </div>
        <div class="field"><label>Webhook / 机器人 URL</label><input name="url" required value="${esc(
          row?.url || ""
        )}" placeholder="https://..." /></div>
        <div class="field"><label>签名密钥（可选）</label><input name="secret" value="${esc(
          row?.secret || ""
        )}" placeholder="钉钉 / 飞书 secret" /></div>
        <div class="field">
          <label class="check-row">
            <input type="checkbox" name="enabled" ${!row || row.enabled ? "checked" : ""} />
            <span>启用此渠道</span>
          </label>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const form = e.target;
      const fd = new FormData(form);
      const secret = String(fd.get("secret") || "").trim();
      const body = {
        name: fd.get("name"),
        kind: fd.get("kind"),
        url: fd.get("url"),
        secret: secret || null,
        enabled: form.querySelector('[name="enabled"]').checked,
      };
      if (row) await api(`/api/channels/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
      else await api("/api/channels", { method: "POST", body: JSON.stringify(body) });
      closeModal();
      toast("已保存");
      renderPage();
    };
  }

  async function editRule(row) {
    const dss = state.cache.datasources || (await api("/api/datasources"));
    const chs = state.cache.channels || (await api("/api/channels"));
    if (!dss.length) {
      toast("请先创建数据源", true);
      return;
    }
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑规则" : "新建告警规则"}</h3>
        <p class="desc">绑定数据源，配置阈值条件与通知渠道。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="seg">
          <div class="seg-title">规则</div>
          <div class="field"><label>名称</label><input name="name" required placeholder="例如：API 不可用" value="${esc(
            row?.name || ""
          )}" /></div>
          <div class="field"><label>数据源</label>
            <select name="datasource_id">${dss
              .map(
                (d) =>
                  `<option value="${d.id}" ${
                    row?.datasource_id === d.id || (!row && d === dss[0]) ? "selected" : ""
                  }>${esc(d.name)} · ${esc(d.kind)}</option>`
              )
              .join("")}</select>
          </div>
          <div class="field"><label>查询表达式</label><input name="expr" required value="${esc(
            row?.expr || ""
          )}" placeholder="PromQL / LogQL / JSON 字段路径" />
            <div class="hint">Prometheus/VM 用 PromQL；Loki 用 LogQL；Kafka 填数值字段路径（可覆盖数据源 field，如 latency_ms）。</div>
          </div>
        </div>
        <div class="seg">
          <div class="seg-title">阈值条件</div>
          <div class="row">
            <div class="field"><label>比较符</label>
              <select name="comparator">
                ${[">", ">=", "<", "<=", "==", "!="]
                  .map(
                    (c) =>
                      `<option value="${c}" ${
                        (row && String(row.comparator) === c) ||
                        (!row && c === ">") ||
                        (row &&
                          { gt: ">", gte: ">=", lt: "<", lte: "<=", eq: "==", neq: "!=" }[
                            row.comparator
                          ] === c)
                          ? "selected"
                          : ""
                      }>${c}</option>`
                  )
                  .join("")}
              </select>
            </div>
            <div class="field"><label>阈值</label><input name="threshold" type="number" step="any" required value="${
              row?.threshold ?? 0
            }" /></div>
          </div>
          <div class="row">
            <div class="field"><label>持续 for（秒）</label><input name="for_seconds" type="number" min="0" value="${
              row?.for_seconds ?? 0
            }" /></div>
            <div class="field"><label>评估间隔（秒）</label><input name="interval_seconds" type="number" min="5" value="${
              row?.interval_seconds ?? 30
            }" /></div>
          </div>
          <div class="row">
            <div class="field"><label>严重级别</label>
              <select name="severity">
                ${["info", "warning", "critical"]
                  .map(
                    (s) =>
                      `<option value="${s}" ${
                        row?.severity === s || (!row && s === "warning") ? "selected" : ""
                      }>${s}</option>`
                  )
                  .join("")}
              </select>
            </div>
          </div>
          <div class="field">
            <label class="check-row">
              <input type="checkbox" name="enabled" ${!row || row.enabled ? "checked" : ""} />
              <span>启用此规则</span>
            </label>
          </div>
        </div>
        <div class="seg">
          <div class="seg-title">通知与标签</div>
          <div class="field"><label>通知渠道</label>
            ${multiSelect(
              "channel_ids",
              chs.map((c) => ({ value: c.id, label: `${c.name} (${c.kind})` })),
              row?.channel_ids || []
            )}
          </div>
          <div class="field"><label>附加标签（可选，JSON）</label>
            <textarea name="labels" rows="2" placeholder='{"team":"sre"}'>${esc(
              Object.keys(row?.labels || {}).length ? JSON.stringify(row.labels, null, 0) : ""
            )}</textarea>
          </div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const form = e.target;
      const fd = new FormData(form);
      let labels = {};
      const rawLabels = String(fd.get("labels") || "").trim();
      if (rawLabels) {
        try {
          labels = JSON.parse(rawLabels);
        } catch {
          toast("标签 JSON 无效", true);
          return;
        }
      }
      const body = {
        name: fd.get("name"),
        datasource_id: fd.get("datasource_id"),
        expr: fd.get("expr"),
        comparator: fd.get("comparator"),
        threshold: Number(fd.get("threshold")),
        for_seconds: Number(fd.get("for_seconds") || 0),
        interval_seconds: Number(fd.get("interval_seconds") || 30),
        severity: fd.get("severity"),
        labels,
        annotations: row?.annotations || {},
        channel_ids: selectedValues(form, "channel_ids"),
        enabled: form.querySelector('[name="enabled"]').checked,
      };
      if (row) await api(`/api/rules/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
      else await api("/api/rules", { method: "POST", body: JSON.stringify(body) });
      closeModal();
      toast("已保存");
      renderPage();
    };
  }

  async function editIngress(row) {
    const chs = state.cache.channels || (await api("/api/channels"));
    const kind = row?.kind || "alertmanager";
    const opt = row?.options || {};
    const types = [
      { id: "alertmanager", name: "Alertmanager", desc: "Prometheus 告警 Webhook" },
      { id: "generic", name: "Generic", desc: "通用 JSON / 拨测 probe-alert" },
      { id: "kafka", name: "Kafka", desc: "从 Topic 消费告警（含拨测 JSON）" },
    ];
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑 Ingress" : "新建 Ingress"}</h3>
        <p class="desc">接入外部已判定的告警，复用去重与通知能力。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field">
          <label>名称</label>
          <input name="name" required placeholder="例如：生产 AM" value="${esc(row?.name || "")}" />
        </div>
        <div class="field">
          <label>类型</label>
          <div class="type-list">
            ${types
              .map(
                (t) => `<label class="type-row">
              <input type="radio" name="kind" value="${t.id}" ${kind === t.id ? "checked" : ""} />
              <span class="t-main">
                <span class="t-name">${t.name}</span>
                <span class="t-desc">${t.desc}</span>
              </span>
            </label>`
              )
              .join("")}
          </div>
        </div>
        <div class="field">
          <label class="check-row">
            <input type="checkbox" name="enabled" ${!row || row.enabled ? "checked" : ""} />
            <span>启用此接入路由</span>
          </label>
        </div>

        <div class="kind-panel" data-kinds="alertmanager,generic" id="ing-http">
          <div class="field">
            <label>鉴权 Token（可选）</label>
            <input name="token" value="${esc(row?.token || "")}" placeholder="请求头 Bearer / X-Eventide-Token" />
            <div class="hint">留空则不校验。保存后在卡片上复制 Webhook，或使用「试推送」验证。</div>
          </div>
          <div class="hint sample-hint" id="ing-format-hint"></div>
        </div>

        <div class="kind-panel" data-kinds="kafka" id="ing-kafka" hidden>
          <div class="field">
            <label>Brokers</label>
            <input name="endpoint" placeholder="127.0.0.1:9092" value="${esc(row?.endpoint || "")}" />
          </div>
          <div class="row">
            <div class="field">
              <label>Topic</label>
              <input name="topic" placeholder="alerts" value="${esc(opt.topic || "")}" />
            </div>
            <div class="field">
              <label>起始位点</label>
              <select name="start">
                <option value="latest" ${!opt.start || opt.start === "latest" ? "selected" : ""}>latest 仅新消息</option>
                <option value="earliest" ${opt.start === "earliest" ? "selected" : ""}>earliest 从头消费</option>
              </select>
            </div>
          </div>
          <div class="field">
            <label>扫描分区数</label>
            <input name="partitions" type="number" min="1" placeholder="8" value="${esc(opt.partitions || "8")}" />
            <div class="hint">也可用 HTTP <code>/api/ingress/{id}/push</code> 测推，无需真实 Topic。</div>
          </div>
        </div>

        <div class="field">
          <label>通知渠道</label>
          ${multiSelect(
            "channel_ids",
            chs.map((c) => ({ value: c.id, label: `${c.name} (${c.kind})` })),
            row?.channel_ids || []
          )}
          <div class="hint">未绑定渠道时告警仍会入库，但不会发送通知。</div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`);

    const form = document.getElementById("f");
    const syncKind = () => {
      const k = form.querySelector('input[name="kind"]:checked')?.value || "alertmanager";
      form.querySelectorAll(".kind-panel").forEach((p) => {
        const kinds = (p.dataset.kinds || "").split(",");
        p.hidden = !kinds.includes(k);
      });
      const hint = document.getElementById("ing-format-hint");
      if (!hint) return;
      if (k === "alertmanager") {
        hint.innerHTML =
          "载荷示例：Alertmanager <code>{\"alerts\":[{\"status\":\"firing\",\"labels\":{...}}]}</code>";
      } else if (k === "generic") {
        hint.innerHTML =
          "支持 Generic 批量告警，或拨测 JSON：<code>{\"eventType\":\"fire\",\"messageId\":\"...\",\"resultFlag\":\"BAD\",...}</code>";
      } else {
        hint.textContent = "";
      }
    };
    form.querySelectorAll('input[name="kind"]').forEach((el) => el.addEventListener("change", syncKind));
    syncKind();

    document.getElementById("m-cancel").onclick = closeModal;
    form.onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(form);
      const k = String(fd.get("kind") || "alertmanager");
      const options = {};
      let endpoint = "";
      let token = String(fd.get("token") || "").trim();
      const channelIds = selectedValues(form, "channel_ids");
      if (!channelIds.length && !confirm("尚未绑定通知渠道，告警只会入库不会通知。仍要保存吗？")) {
        return;
      }
      if (k === "kafka") {
        endpoint = String(fd.get("endpoint") || "").trim();
        const topic = String(fd.get("topic") || "").trim();
        if (!endpoint || !topic) {
          toast("请填写 Brokers 与 Topic", true);
          return;
        }
        options.topic = topic;
        options.start = String(fd.get("start") || "latest");
        const parts = String(fd.get("partitions") || "").trim();
        if (parts) options.partitions = parts;
        token = "";
      }
      const body = {
        name: fd.get("name"),
        kind: k,
        token: token || null,
        endpoint,
        options,
        channel_ids: channelIds,
        enabled: form.querySelector('[name="enabled"]').checked,
      };
      let saved;
      if (row) {
        saved = await api(`/api/ingress/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
      } else {
        saved = await api("/api/ingress", { method: "POST", body: JSON.stringify(body) });
      }
      closeModal();
      showIngressSaved(saved || { ...body, id: row?.id });
    };
  }

  function showIngressSaved(route) {
    if (!route?.id) {
      toast("已保存");
      renderPage();
      return;
    }
    const origin = location.origin;
    const mainUrl =
      route.kind === "kafka"
        ? `kafka://${route.endpoint || ""}/${(route.options && route.options.topic) || ""}`
        : route.kind === "alertmanager"
        ? `${origin}/api/ingress/${route.id}/alertmanager`
        : `${origin}/api/ingress/${route.id}/generic`;
    const pushUrl = `${origin}/api/ingress/${route.id}/push`;
    const tokenHint = route.token
      ? `-H "Authorization: Bearer <token>"`
      : "# 未配置 Token，可直接推送";
    openModal(`
      <div class="modal-head">
        <h3>Ingress 已保存</h3>
        <p class="desc">${esc(route.name)} · ${esc(route.kind)}。复制地址或试推送验证。</p>
      </div>
      <div class="modal-body">
        <div class="field">
          <label>主接入地址</label>
          <div class="url-row">
            <code class="mono url-box">${esc(mainUrl)}</code>
            <button type="button" data-copy="${esc(mainUrl)}">复制</button>
          </div>
        </div>
        <div class="field">
          <label>自动识别 / 测推</label>
          <div class="url-row">
            <code class="mono url-box">${esc(pushUrl)}</code>
            <button type="button" data-copy="${esc(pushUrl)}">复制</button>
          </div>
          <div class="hint">curl 示例：<code class="mono">curl -X POST ${esc(pushUrl)} ${esc(
            tokenHint
          )} -H "Content-Type: application/json" -d "{...}"</code></div>
        </div>
      </div>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-test">试推送</button>
        <button type="button" class="primary" id="m-close">完成</button>
      </div>`);
    document.querySelectorAll("#modal [data-copy]").forEach((b) => {
      b.onclick = async () => {
        await navigator.clipboard.writeText(b.dataset.copy);
        toast("已复制");
      };
    });
    document.getElementById("m-close").onclick = () => {
      closeModal();
      renderPage();
    };
    document.getElementById("m-test").onclick = () => {
      closeModal();
      openIngressTest(route);
    };
  }

  function editSilence(prefill) {
    const now = new Date();
    const end = new Date(now.getTime() + 2 * 3600 * 1000);
    const toLocal = (d) => {
      const pad = (n) => String(n).padStart(2, "0");
      return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(
        d.getHours()
      )}:${pad(d.getMinutes())}`;
    };
    const matchersJson = JSON.stringify(prefill?.matchers || {}, null, 2);
    openModal(`
      <div class="modal-head">
        <h3>新建静默</h3>
        <p class="desc">在时间窗内抑制匹配标签的通知。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>注释</label><input name="comment" placeholder="维护窗口" value="${esc(
          prefill?.comment || ""
        )}" /></div>
        <div class="field"><label>规则 ID（可选，留空匹配全部）</label><input name="rule_id" placeholder="uuid" value="${esc(
          prefill?.rule_id || ""
        )}" /></div>
        <div class="field"><label>匹配标签 JSON</label>
          <textarea name="matchers" rows="4">${esc(matchersJson)}</textarea>
        </div>
        <div class="row">
          <div class="field"><label>开始</label><input name="starts_at" type="datetime-local" required value="${toLocal(
            now
          )}" /></div>
          <div class="field"><label>结束</label><input name="ends_at" type="datetime-local" required value="${toLocal(
            end
          )}" /></div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(e.target);
      let matchers = {};
      try {
        matchers = JSON.parse(String(fd.get("matchers") || "{}"));
      } catch {
        toast("匹配标签 JSON 无效", true);
        return;
      }
      const ruleId = String(fd.get("rule_id") || "").trim();
      const body = {
        comment: fd.get("comment") || "",
        rule_id: ruleId || null,
        matchers,
        starts_at: new Date(fd.get("starts_at")).toISOString(),
        ends_at: new Date(fd.get("ends_at")).toISOString(),
      };
      await api("/api/silences", { method: "POST", body: JSON.stringify(body) });
      closeModal();
      toast("已保存");
      renderPage();
    };
  }

  tryBoot();
})();
