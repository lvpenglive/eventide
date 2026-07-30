/* Eventide console SPA */
(() => {
  const TOKEN_KEY = "eventide_token";
  const USER_KEY = "eventide_user";
  const SIDEBAR_KEY = "eventide_sidebar_collapsed";
  const NAV_GROUPS_KEY = "eventide_nav_groups";
  const ALERT_VIEW_KEY = "eventide_alert_view"; // cards | table
  const THEME_KEY = "eventide_theme"; // light | dark | system

  // Zabbix-aligned severities (0–5)
  const SEVERITIES = [
    ["not_classified", "0 Not classified"],
    ["information", "1 Information"],
    ["warning", "2 Warning"],
    ["average", "3 Average"],
    ["high", "4 High"],
    ["disaster", "5 Disaster"],
  ];
  function severityOptions(selected, fallback = "warning") {
    const cur = selected || fallback;
    return SEVERITIES.map(
      ([v, label]) =>
        `<option value="${v}" ${cur === v ? "selected" : ""}>${label}</option>`
    ).join("");
  }

  const PAGE_GROUP = {
    overview: null,
    alerts: "ops",
    silences: "ops",
    datasources: "config",
    rules: "config",
    ingress: "config",
    trap: "config",
    mib: "config",
    policies: "config",
    channels: "notify",
    notifies: "notify",
    enrich: "notify",
    users: "system",
    roles: "system",
    departments: "system",
    settings: "system",
  };

  const PAGE_PERM = {
    overview: "overview:read",
    alerts: "alerts:read",
    silences: "silences:read",
    datasources: "datasources:read",
    rules: "rules:read",
    ingress: "ingress:read",
    trap: "trap:read",
    mib: "trap:read",
    policies: "trap:read",
    channels: "channels:read",
    notifies: "channels:read",
    enrich: "enrich:read",
    users: "users:read",
    roles: "roles:read",
    departments: "departments:read",
    settings: "settings:read",
  };

  const PAGE_ORDER = [
    "overview",
    "alerts",
    "silences",
    "datasources",
    "rules",
    "ingress",
    "trap",
    "mib",
    "policies",
    "channels",
    "notifies",
    "enrich",
    "users",
    "roles",
    "departments",
    "settings",
  ];

  const state = {
    page: "overview",
    cache: {},
    me: null,
  };

  const titles = {
    overview: ["总览", "系统运行状态与近期告警"],
    datasources: ["数据源", "Prometheus / VictoriaMetrics"],
    rules: ["告警规则", "阈值评估与通知绑定"],
    channels: ["通知渠道", "Webhook · 自定义 HTTP · 钉钉 · 企微 · 飞书 · Slack · TG"],
    notifies: ["通知日志", "发送记录 · 可查正文"],
    ingress: ["告警接入", "外部告警接入 · 试推送 · 通知绑定"],
    trap: ["SNMP Trap", "Trap 服务状态 · 试推送 · 对接 Kafka Ingress"],
    mib: ["MIB 库", "上传 MIB · OID 树浏览 · 导出 Trap 策略"],
    policies: ["Trap 策略", "导入 MIB 导出 · 匹配 OID · 摘要模板生效"],
    alerts: ["告警事件", "按状态浏览 · 点开看详情"],
    enrich: ["告警丰富", "台账补字段 · 写描述 / IP / 级别"],
    silences: ["静默策略", "按规则或标签临时抑制通知"],
    users: ["用户管理", "账号 · 部门 · 角色"],
    roles: ["权限管理", "角色与权限码"],
    departments: ["部门管理", "组织架构"],
    settings: ["系统设置", "外观 · 告警历史 · 运行信息"],
  };

  function can(perm) {
    const perms = (state.me && state.me.permissions) || [];
    return perms.includes("*") || perms.includes(perm);
  }

  function canPage(page) {
    const need = PAGE_PERM[page];
    return !need || can(need);
  }

  function firstAllowedPage() {
    return PAGE_ORDER.find((p) => canPage(p)) || "settings";
  }

  function applyNavPermissions() {
    document.querySelectorAll(".nav-item[data-page]").forEach((btn) => {
      const ok = canPage(btn.dataset.page);
      btn.hidden = !ok;
      if (!ok) btn.classList.remove("active");
    });
    document.querySelectorAll(".nav-group").forEach((g) => {
      const any = [...g.querySelectorAll(".nav-item[data-page]")].some((b) => !b.hidden);
      g.hidden = !any;
    });
  }

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
    applyNavPermissions();
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

  // ---------- Theme ----------
  function themePref() {
    const p = localStorage.getItem(THEME_KEY);
    return p === "light" || p === "dark" ? p : "system";
  }

  function resolveTheme(pref) {
    const p = pref || themePref();
    if (p === "light" || p === "dark") return p;
    return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
  }

  function applyTheme(pref) {
    const p = pref === "light" || pref === "dark" ? pref : "system";
    localStorage.setItem(THEME_KEY, p);
    const resolved = resolveTheme(p);
    document.documentElement.setAttribute("data-theme", resolved);
    document.documentElement.setAttribute("data-theme-pref", p);
    syncThemeControls();
  }

  function themeSwitchHtml(mode) {
    const pref = themePref();
    const items =
      mode === "cards"
        ? [
            ["light", "浅色", "明亮工作台"],
            ["dark", "深色", "夜间护眼"],
            ["system", "跟随系统", "自动匹配 OS"],
          ]
        : [
            ["light", "☀", "浅色"],
            ["dark", "☾", "深色"],
            ["system", "◐", "跟随系统"],
          ];
    if (mode === "cards") {
      return `<div class="theme-cards">${items
        .map(
          ([v, title, desc]) =>
            `<button type="button" class="theme-card${pref === v ? " on" : ""}" data-theme-set="${v}">
              <strong>${title}</strong><span>${desc}</span>
            </button>`
        )
        .join("")}</div>`;
    }
    return items
      .map(
        ([v, label, title]) =>
          `<button type="button" class="${pref === v ? "on" : ""}" data-theme-set="${v}" title="${title}">${label}</button>`
      )
      .join("");
  }

  function bindThemeHost(el, mode) {
    if (!el) return;
    el.innerHTML = themeSwitchHtml(mode);
    el.querySelectorAll("[data-theme-set]").forEach((btn) => {
      btn.addEventListener("click", () => applyTheme(btn.dataset.themeSet));
    });
  }

  function syncThemeControls() {
    bindThemeHost(document.getElementById("login-theme"));
    bindThemeHost(document.getElementById("sidebar-theme"));
    const settingsHost = document.getElementById("settings-theme");
    if (settingsHost) bindThemeHost(settingsHost, "cards");
  }

  bindThemeHost(document.getElementById("login-theme"));
  bindThemeHost(document.getElementById("sidebar-theme"));
  if (window.matchMedia) {
    matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      if (themePref() === "system") applyTheme("system");
    });
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
      const page = canPage(state.page) ? state.page : firstAllowedPage();
      navigate(page);
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
      navigate(firstAllowedPage());
      toast("登录成功");
    } catch (ex) {
      err.textContent = ex.message || "登录失败";
    }
  });

  document.getElementById("btn-logout").addEventListener("click", () => logout());
  document.getElementById("btn-sidebar").addEventListener("click", () => toggleSidebar());

  // ---------- Navigation ----------
  function loadNavGroups() {
    try {
      return JSON.parse(localStorage.getItem(NAV_GROUPS_KEY) || "{}");
    } catch {
      return {};
    }
  }

  function saveNavGroups(map) {
    localStorage.setItem(NAV_GROUPS_KEY, JSON.stringify(map));
  }

  function applyNavGroups(forceOpenGroup) {
    const saved = loadNavGroups();
    document.querySelectorAll(".nav-group").forEach((g) => {
      const id = g.dataset.group;
      let open = saved[id];
      if (open === undefined) open = true; // default expanded
      if (forceOpenGroup && id === forceOpenGroup) open = true;
      g.classList.toggle("open", !!open);
    });
  }

  document.querySelectorAll(".nav-item").forEach((btn) => {
    btn.addEventListener("click", () => navigate(btn.dataset.page));
  });
  document.querySelectorAll("[data-group-toggle]").forEach((btn) => {
    btn.addEventListener("click", () => {
      const id = btn.dataset.groupToggle;
      const group = document.querySelector(`.nav-group[data-group="${id}"]`);
      if (!group) return;
      const next = !group.classList.contains("open");
      group.classList.toggle("open", next);
      const saved = loadNavGroups();
      saved[id] = next;
      saveNavGroups(saved);
    });
  });

  let overviewTimer = null;
  function stopOverviewTimer() {
    if (overviewTimer) {
      clearInterval(overviewTimer);
      overviewTimer = null;
    }
  }

  function navigate(page) {
    if (!canPage(page)) {
      page = firstAllowedPage();
    }
    if (page !== "overview") stopOverviewTimer();
    state.page = page;
    applyNavPermissions();
    document.querySelectorAll(".nav-item").forEach((b) => {
      b.classList.toggle("active", b.dataset.page === page);
    });
    const gid = PAGE_GROUP[page];
    applyNavGroups(gid || undefined);
    // mark group containing active page
    document.querySelectorAll(".nav-group").forEach((g) => {
      const hasActive = !!g.querySelector(`.nav-item.active`);
      g.classList.toggle("has-active", hasActive);
    });
    const [t, s] = titles[page] || [page, ""];
    document.getElementById("page-title").textContent = t;
    document.getElementById("page-sub").textContent = s;
    document.getElementById("page-actions").innerHTML = "";
    renderPage();
  }

  applyNavGroups();

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

  function openModal(html, opts = {}) {
    modalBody.classList.remove("wide", "xl");
    // Default all modals to wide; pass { xl: true } for large forms.
    if (opts.xl) modalBody.classList.add("xl");
    else modalBody.classList.add("wide");
    modalBody.innerHTML = html;
    modal.classList.add("open");
  }
  function closeModal() {
    modal.classList.remove("open");
    modalBody.classList.remove("wide", "xl");
    modalBody.innerHTML = "";
  }
  /** @returns {Promise<null|'merge'|'skip'>} */
  function askPolicyImportMode(title = "导入策略") {
    return new Promise((resolve) => {
      openModal(`
        <div class="modal-head"><h3>${esc(title)}</h3>
          <p class="desc">「合并导入」：同 OID 更新、新 OID 追加。<br/>
          「忽略重复」：已有同 OID 的跳过，只追加新的。</p>
        </div>
        <div class="modal-foot">
          <button type="button" class="ghost" id="imp-cancel">取消</button>
          <button type="button" class="ghost" id="imp-skip">忽略重复</button>
          <button type="button" class="primary" id="imp-merge">合并导入</button>
        </div>
      `);
      const done = (v) => {
        closeModal();
        resolve(v);
      };
      document.getElementById("imp-cancel").onclick = () => done(null);
      document.getElementById("imp-skip").onclick = () => done("skip");
      document.getElementById("imp-merge").onclick = () => done("merge");
    });
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
      setActions(
        `<span class="overview-refresh-hint" id="ov-hint"></span><button class="ghost" id="btn-refresh">刷新</button>`
      );
      document.getElementById("btn-refresh").onclick = () => paint();

      const goAlerts = (status) => {
        state.alertFilters = {
          status: status || "",
          severity: "",
          source: "",
          q: "",
        };
        navigate("alerts");
      };

      const paint = async () => {
        const d = await api("/api/overview");
        const ingressMap = Object.fromEntries(
          (d.ingress || []).map((r) => [r.id, r])
        );
        const firing = d.alerts_firing || 0;
        const pending = d.alerts_pending || 0;
        const resolved = d.alerts_resolved || 0;
        const recent = d.recent_alerts || [];
        const skips = d.notify_skips || [];
        const needsSetup =
          !d.datasources || !d.enabled_rules || !d.channels || !d.ingress_routes;

        const hint = document.getElementById("ov-hint");
        if (hint) {
          hint.textContent = "每 30 秒自动刷新";
        }

        root.innerHTML = `
          <div class="overview-health ${firing ? "is-firing" : "is-ok"}">
            <div class="overview-health-main">${esc(d.health || "—")}</div>
            <div class="overview-health-actions">
              ${
                firing
                  ? `<button type="button" class="primary" data-go-alerts="firing">查看正在告警</button>`
                  : `<button type="button" class="ghost" data-go-alerts="">打开告警事件</button>`
              }
            </div>
          </div>

          <div class="stats overview-stats-primary">
            <button type="button" class="stat firing clickable" data-go-alerts="firing">
              <div class="n">${firing}</div><div class="l">正在告警</div>
            </button>
            <button type="button" class="stat clickable" data-go-alerts="pending">
              <div class="n">${pending}</div><div class="l">等待中</div>
            </button>
            <button type="button" class="stat ok clickable" data-go-alerts="resolved">
              <div class="n">${resolved}</div><div class="l">已恢复</div>
            </button>
            <button type="button" class="stat accent clickable" data-go-page="silences">
              <div class="n">${d.active_silences || 0}</div><div class="l">生效静默</div>
            </button>
          </div>

          <div class="stats overview-stats-meta">
            <button type="button" class="stat clickable" data-go-page="rules">
              <div class="n">${d.enabled_rules || 0}/${d.rules || 0}</div><div class="l">启用规则</div>
            </button>
            <button type="button" class="stat clickable" data-go-page="datasources">
              <div class="n">${d.datasources || 0}</div><div class="l">数据源</div>
            </button>
            <button type="button" class="stat clickable" data-go-page="channels">
              <div class="n">${d.channels || 0}</div><div class="l">通知渠道</div>
            </button>
            <button type="button" class="stat clickable" data-go-page="ingress">
              <div class="n">${d.ingress_routes || 0}</div><div class="l">告警接入</div>
            </button>
          </div>

          ${
            needsSetup
              ? `<div class="panel overview-setup">
                  <strong>尚未完成最小闭环</strong>
                  <p class="hint">建议先准备数据源或告警接入、通知渠道，再配置规则 / 丰富。</p>
                  <div class="overview-setup-actions">
                    ${!d.datasources ? `<button type="button" class="ghost" data-go-page="datasources">新建数据源</button>` : ""}
                    ${!d.ingress_routes ? `<button type="button" class="ghost" data-go-page="ingress">配置告警接入</button>` : ""}
                    ${!d.channels ? `<button type="button" class="ghost" data-go-page="channels">新建通知渠道</button>` : ""}
                    ${!d.enabled_rules ? `<button type="button" class="ghost" data-go-page="rules">新建规则</button>` : ""}
                  </div>
                </div>`
              : ""
          }

          <div class="overview-grid">
            <div class="panel">
              <div class="overview-panel-head">
                <h3>关注中的告警</h3>
                <button type="button" class="ghost" data-go-alerts="firing">全部告警中</button>
              </div>
              ${
                recent.length
                  ? alertTable(recent, { compact: true, ingressMap })
                  : `<div class="empty">暂无告警。可去「告警接入」试推送，或等待规则触发。
                      <div style="margin-top:12px"><button type="button" class="primary" data-go-page="ingress">去告警接入</button></div>
                    </div>`
              }
            </div>
            <div class="panel">
              <div class="overview-panel-head">
                <h3>最近通知跳过</h3>
                <span class="hint">节流 / 聚合 / 降级</span>
              </div>
              ${
                skips.length
                  ? `<ul class="overview-skip-list">${skips
                      .map(
                        (s) =>
                          `<li><code class="mono">${esc(s.error || "—")}</code>
                            <span class="hint">${esc(s.transition || "")} · ${esc(
                            (s.created_at || "").replace("T", " ").slice(0, 19)
                          )}</span></li>`
                      )
                      .join("")}</ul>`
                  : `<div class="empty hint">暂无跳过记录。风暴节流生效时会出现 throttled / aggregated / degraded。</div>`
              }
            </div>
          </div>`;

        root.querySelectorAll("[data-go-alerts]").forEach((el) => {
          el.onclick = () => goAlerts(el.dataset.goAlerts || "");
        });
        root.querySelectorAll("[data-go-page]").forEach((el) => {
          el.onclick = () => navigate(el.dataset.goPage);
        });
        if (recent.length) bindAlertTable(root, recent, ingressMap);
      };

      await paint();
      stopOverviewTimer();
      overviewTimer = setInterval(() => {
        if (state.page !== "overview") {
          stopOverviewTimer();
          return;
        }
        paint().catch(() => {});
      }, 30000);
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
                <button data-test="${c.id}">测试</button>
                <button data-edit="${c.id}">编辑</button>
                <button class="danger" data-del="${c.id}">删除</button>
              </td></tr>`
              )
              .join("")}</tbody></table>`
          : `<div class="empty">还没有通知渠道。支持 webhook / http / dingtalk / wecom / feishu / slack / telegram。</div>`
      }</div>`;
      root.querySelectorAll("[data-test]").forEach((b) => {
        b.onclick = () => testChannel(b.dataset.test, b);
      });
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

    async notifies(root) {
      const f = state.notifyFilters || { channel_id: "", success: "", q: "" };
      setActions(`<button class="ghost" id="btn-refresh">刷新</button>`);

      const load = async () => {
        const channel_id =
          root.querySelector("#nf-channel")?.value ?? state.notifyFilters?.channel_id ?? "";
        const success =
          root.querySelector("#nf-success")?.value ?? state.notifyFilters?.success ?? "";
        const q = (root.querySelector("#nf-q")?.value ?? state.notifyFilters?.q ?? "").trim();
        state.notifyFilters = { channel_id, success, q };

        const params = new URLSearchParams();
        if (channel_id) params.set("channel_id", channel_id);
        if (success) params.set("success", success);
        if (q) params.set("q", q);
        params.set("limit", "200");

        const [rows, chs] = await Promise.all([
          api("/api/notifies?" + params.toString()),
          api("/api/channels").catch(() => state.cache.channels || []),
        ]);
        state.cache.channels = chs;

        const preview = (body) => {
          const s = String(body || "").replace(/\s+/g, " ").trim();
          if (!s) return "—";
          return s.length > 72 ? s.slice(0, 72) + "…" : s;
        };
        const edgeLabel = (t, err) => {
          if (t === "became_firing") return "触发";
          if (t === "became_resolved") return "恢复";
          if (err === "test" || String(err || "").startsWith("test")) return "测试";
          return t || "其他";
        };

        root.innerHTML = `
          <div class="panel">
            <div class="alert-toolbar">
              <div class="alert-filters">
                <select id="nf-channel">
                  <option value="">全部渠道</option>
                  ${(chs || [])
                    .map(
                      (c) =>
                        `<option value="${esc(c.id)}" ${
                          channel_id === c.id ? "selected" : ""
                        }>${esc(c.name)} (${esc(c.kind)})</option>`
                    )
                    .join("")}
                </select>
                <select id="nf-success">
                  <option value="" ${!success ? "selected" : ""}>全部结果</option>
                  <option value="true" ${success === "true" ? "selected" : ""}>成功</option>
                  <option value="false" ${success === "false" ? "selected" : ""}>失败</option>
                </select>
                <input id="nf-q" type="search" placeholder="搜索正文 / 错误 / 告警 ID" value="${esc(
                  q
                )}" />
              </div>
            </div>
            ${
              rows.length
                ? `<table class="data"><thead><tr>
                    <th>时间</th><th>渠道</th><th>边沿</th><th>结果</th><th>内容摘要</th><th>错误</th><th></th>
                  </tr></thead><tbody>${rows
                    .map(
                      (n, i) => `<tr>
                    <td>${esc(fmtTime(n.created_at))}</td>
                    <td>${esc(n.channel_name || n.channel_id?.slice?.(0, 8) || "—")}<div class="hint" style="margin:0">${esc(
                      n.channel_kind || ""
                    )}</div></td>
                    <td>${esc(edgeLabel(n.transition, n.error))}</td>
                    <td><span class="badge ${n.success ? "on" : "firing"}">${
                      n.success ? "成功" : "失败"
                    }</span></td>
                    <td class="mono" title="${esc(n.body || "")}">${esc(preview(n.body))}</td>
                    <td class="mono">${esc(n.error || "—")}</td>
                    <td class="actions"><button data-body-idx="${i}">查看内容</button></td>
                  </tr>`
                    )
                    .join("")}</tbody></table>`
                : `<div class="empty">暂无通知记录。告警边沿通知或渠道测试后会出现在这里。</div>`
            }
          </div>`;

        root.querySelector("#nf-channel").onchange = () => load();
        root.querySelector("#nf-success").onchange = () => load();
        const qEl = root.querySelector("#nf-q");
        let t;
        qEl.oninput = () => {
          clearTimeout(t);
          t = setTimeout(() => load(), 280);
        };
        root.querySelectorAll("[data-body-idx]").forEach((b) => {
          b.onclick = () => showNotifyLogDetail(rows[Number(b.dataset.bodyIdx)]);
        });
      };

      document.getElementById("btn-refresh").onclick = () => load();
      state.notifyFilters = { ...f };
      await load();
    },

    async ingress(root) {
      setActions(`<button class="primary" id="btn-add">新建接入</button>`);
      document.getElementById("btn-add").onclick = () => editIngress();
      const [rows, chs] = await Promise.all([api("/api/ingress"), api("/api/channels")]);
      state.cache.channels = chs;
      const chMap = Object.fromEntries(chs.map((c) => [c.id, c]));
      const origin = location.origin;

      if (!rows.length) {
        root.innerHTML = `
          <div class="panel guide">
            <h3>配置告警接入</h3>
            <ol class="steps">
              <li>先在「通知渠道」配置至少一个机器人 / Webhook（可选，也可稍后绑定）</li>
              <li>创建接入：选择 Alertmanager / Generic（含拨测） / Kafka</li>
              <li>把外部平台 Webhook 指到下方生成的地址，或使用「试推送」验证</li>
              <li>在「告警事件」查看 firing / resolved 与通知结果</li>
            </ol>
            <button class="primary" id="btn-guide-add">新建第一个接入</button>
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
          if (!confirm("确认删除该接入？")) return;
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
      const f = state.alertFilters || { status: "", severity: "", source: "", q: "", store: "" };
      setActions(`<button class="ghost" id="btn-refresh">刷新</button>`);

      const load = async () => {
        const status = state.alertFilters?.status || "";
        const severity = root.querySelector("#alert-sev")?.value ?? state.alertFilters?.severity ?? "";
        const source = root.querySelector("#alert-source")?.value ?? state.alertFilters?.source ?? "";
        const q = (root.querySelector("#alert-q")?.value ?? state.alertFilters?.q ?? "").trim();
        const store =
          root.querySelector("#alert-store")?.value ?? state.alertFilters?.store ?? "";
        state.alertFilters = { status, severity, source, q, store };

        const params = new URLSearchParams();
        if (status) params.set("status", status);
        if (severity) params.set("severity", severity);
        if (source) params.set("source", source);
        if (q) params.set("q", q);
        if (store) params.set("store", store);

        const countParams = new URLSearchParams();
        if (store) countParams.set("store", store);

        const [rows, allRows, ingressRows] = await Promise.all([
          api("/api/alerts" + (params.toString() ? `?${params}` : "")),
          api("/api/alerts" + (countParams.toString() ? `?${countParams}` : "")).catch(() => []),
          api("/api/ingress").catch(() => []),
        ]);
        const ingressMap = Object.fromEntries((ingressRows || []).map((r) => [r.id, r]));
        const nFire = allRows.filter((a) => a.status === "firing").length;
        const nPend = allRows.filter((a) => a.status === "pending").length;
        const nRes = allRows.filter((a) => a.status === "resolved").length;

        const view = getAlertView();
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
        root.querySelector("#alert-store").onchange = (e) => {
          state.alertFilters = { ...(state.alertFilters || {}), store: e.target.value };
          load();
        };
      };

      document.getElementById("btn-refresh").onclick = () => load();
      // seed filters from initial f
      state.alertFilters = { ...f };
      await load();
    },

    async enrich(root) {
      const ENRICH_TAB_KEY = "eventide_enrich_tab";
      const [rows, lookups] = await Promise.all([api("/api/enrich"), api("/api/lookups")]);
      state.cache.lookups = lookups;
      let tab = localStorage.getItem(ENRICH_TAB_KEY) || "rules";
      if (tab !== "rules" && tab !== "lookups") tab = "rules";
      // first visit with no ledger: nudge to lookups
      if (!lookups.length && !rows.length && !localStorage.getItem(ENRICH_TAB_KEY)) {
        tab = "lookups";
      }

      const lookupName = (id) => {
        const t = lookups.find((x) => x.id === id);
        return t ? t.name : id ? id.slice(0, 8) + "…" : "—";
      };
      const enrichWhat = (r) => {
        const parts = [];
        const ids =
          r.lookup_table_ids && r.lookup_table_ids.length
            ? r.lookup_table_ids
            : r.lookup_table_id
            ? [r.lookup_table_id]
            : [];
        if (ids.length) {
          parts.push(`查 ${ids.map(lookupName).join("、")}`);
        }
        if (r.mappings && Object.keys(r.mappings).length) {
          parts.push(`内联映射 ${Object.keys(r.mappings).length} 条`);
        }
        const ft = r.field_templates || {};
        const writes = [];
        if (ft.summary || (r.templates && r.templates.summary)) writes.push("描述");
        if (ft.ip || ft.alertIp) writes.push("IP");
        if (ft.severity) writes.push("级别");
        if (ft.alertname) writes.push("名称");
        if (writes.length) parts.push(`写${writes.join("/")}`);
        else if (r.templates && Object.keys(r.templates).length) {
          parts.push("写注解模板");
        }
        return parts.length ? parts.join(" · ") : "未配置动作";
      };
      const enrichScope = (r) => {
        const m = r.matchers || {};
        const keys = Object.keys(m);
        if (!keys.length) return "全部告警";
        return keys.map((k) => `${k}=${m[k]}`).join("，");
      };

      const setTab = (next) => {
        localStorage.setItem(ENRICH_TAB_KEY, next);
        renderPage();
      };

      const syncActions = () => {
        if (tab === "lookups") {
          setActions(`<button class="primary" id="btn-add-lookup">新建台账</button>`);
          document.getElementById("btn-add-lookup").onclick = () => editLookup();
        } else {
          setActions(`<button class="ghost" id="btn-preview">试跑预览</button><button class="primary" id="btn-add">新建丰富规则</button>`);
          document.getElementById("btn-preview").onclick = () => openEnrichPreviewModal({});
          document.getElementById("btn-add").onclick = () => {
            if (!lookups.length) {
              toast("请先在「台账数据」里准备一张表", true);
              setTab("lookups");
              return;
            }
            editEnrich();
          };
        }
      };
      syncActions();

      const rulesPanel = rows.length
        ? `<table class="data"><thead><tr>
              <th>名称</th><th>做什么</th><th>作用范围</th><th>状态</th><th></th>
            </tr></thead>
            <tbody>${rows
              .map((r) => `<tr>
              <td>${esc(r.name)}</td>
              <td>${esc(enrichWhat(r))}</td>
              <td>${esc(enrichScope(r))}</td>
              <td><span class="badge ${r.enabled ? "on" : "off"}">${
                r.enabled ? "启用" : "停用"
              }</span></td>
              <td class="actions">
                <button data-edit="${r.id}">编辑</button>
                <button class="danger" data-del="${r.id}">删除</button>
              </td>
            </tr>`)
              .join("")}</tbody></table>`
        : `<div class="enrich-empty">
            <p><strong>还没有丰富规则</strong></p>
            <p class="hint">规则会把台账里的主机名、联系人等信息写到告警描述 / IP / 级别上。</p>
            ${
              lookups.length
                ? `<button class="primary" id="btn-empty-rule">新建第一条规则</button>`
                : `<button class="primary" id="btn-empty-to-lookup">先去准备台账</button>`
            }
          </div>`;

      const lookupsPanel = lookups.length
        ? `<table class="data"><thead><tr>
              <th>名称</th><th>用哪个标签匹配</th><th>行数</th><th>说明</th><th>状态</th><th></th>
            </tr></thead>
            <tbody>${lookups
              .map(
                (t) => `<tr>
              <td>${esc(t.name)}</td>
              <td class="mono">${esc(t.key_label || "ip")}</td>
              <td>${Object.keys(t.rows || {}).length}</td>
              <td>${esc(t.description || "—")}</td>
              <td><span class="badge ${t.enabled ? "on" : "off"}">${
                t.enabled ? "启用" : "停用"
              }</span></td>
              <td class="actions">
                <button data-edit-lookup="${t.id}">编辑</button>
                <button class="danger" data-del-lookup="${t.id}">删除</button>
              </td>
            </tr>`
              )
              .join("")}</tbody></table>`
        : `<div class="enrich-empty">
            <p><strong>还没有台账数据</strong></p>
            <p class="hint">台账像一张对照表：用告警里的 IP（或其它标签）查出主机名、机房、联系人等，再写回告警。</p>
            <button class="primary" id="btn-empty-lookup">新建台账</button>
          </div>`;

      root.innerHTML = `
        <div class="enrich-guide panel">
          <div class="enrich-guide-steps">
            <div class="enrich-guide-step"><span class="n">1</span><div><strong>准备台账</strong><p>导入主机 / 设备对照表</p></div></div>
            <div class="enrich-guide-step"><span class="n">2</span><div><strong>建丰富规则</strong><p>勾选要用的台账</p></div></div>
            <div class="enrich-guide-step"><span class="n">3</span><div><strong>写到告警上</strong><p>配置描述、IP、级别；保存前可弹窗试跑</p></div></div>
          </div>
          ${
            !lookups.length
              ? `<button class="primary" id="btn-guide-start">从台账开始</button>`
              : `<button class="primary" id="btn-guide-rule">新建丰富规则</button>`
          }
        </div>
        <div class="enrich-tabs" role="tablist">
          <button type="button" class="enrich-tab ${
            tab === "rules" ? "on" : ""
          }" data-tab="rules" role="tab">丰富规则${
            rows.length ? ` · ${rows.length}` : ""
          }</button>
          <button type="button" class="enrich-tab ${
            tab === "lookups" ? "on" : ""
          }" data-tab="lookups" role="tab">台账数据${
            lookups.length ? ` · ${lookups.length}` : ""
          }</button>
        </div>
        <div class="panel enrich-tab-panel">
          ${tab === "rules" ? rulesPanel : lookupsPanel}
        </div>`;

      root.querySelectorAll(".enrich-tab").forEach((btn) => {
        btn.onclick = () => setTab(btn.dataset.tab);
      });
      const guideStart = document.getElementById("btn-guide-start");
      if (guideStart) guideStart.onclick = () => setTab("lookups");
      const guideRule = document.getElementById("btn-guide-rule");
      if (guideRule) guideRule.onclick = () => editEnrich();
      const emptyRule = document.getElementById("btn-empty-rule");
      if (emptyRule) emptyRule.onclick = () => editEnrich();
      const emptyToLookup = document.getElementById("btn-empty-to-lookup");
      if (emptyToLookup) emptyToLookup.onclick = () => setTab("lookups");
      const emptyLookup = document.getElementById("btn-empty-lookup");
      if (emptyLookup) emptyLookup.onclick = () => editLookup();

      const byId = Object.fromEntries(rows.map((r) => [r.id, r]));
      const lookupById = Object.fromEntries(lookups.map((t) => [t.id, t]));
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editEnrich(byId[b.dataset.edit]);
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该丰富规则？")) return;
          await api(`/api/enrich/${b.dataset.del}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
      root.querySelectorAll("[data-edit-lookup]").forEach((b) => {
        b.onclick = () => editLookup(lookupById[b.dataset.editLookup]);
      });
      root.querySelectorAll("[data-del-lookup]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该台账？引用它的丰富规则将失效。")) return;
          await api(`/api/lookups/${b.dataset.delLookup}`, { method: "DELETE" });
          toast("已删除");
          renderPage();
        };
      });
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

    async trap(root) {
      setActions(`<button class="ghost" id="btn-trap-refresh">刷新</button>`);
      const canWrite = can("trap:write");
      const paint = async () => {
        let health = null;
        let stats = null;
        let recent = { items: [] };
        let err = null;
        try {
          health = await api("/trap-api/api/health");
          stats = await api("/trap-api/api/stats");
          recent = await api("/trap-api/api/recent");
        } catch (e) {
          err = e.message || String(e);
        }

        if (err) {
          root.innerHTML = `
            <div class="panel" style="max-width:720px">
              <h3 style="margin:0 0 0.75rem;font-size:1rem">Trap 服务未连通</h3>
              <p class="hint">${esc(err)}</p>
              <p class="hint" style="margin-top:12px">
                1. 启动 Trap：<code>cargo run -p eventide-trap -- eventide-trap.toml</code><br/>
                2. 在 <code>eventide.toml</code> 配置 <code>[trap] api_url = "http://127.0.0.1:8081"</code><br/>
                3. Eventide 侧配置 Kafka Ingress，Topic 与 Trap 写出一致（默认 <code>eventide.snmptrap</code>）
              </p>
            </div>`;
          return;
        }

        const items = recent.items || [];
        root.innerHTML = `
          <div class="panel" style="max-width:900px;margin-bottom:16px">
            <h3 style="margin:0 0 0.75rem;font-size:1rem">服务状态</h3>
            <p class="hint" style="margin:0 0 12px">
              <span class="badge on">在线</span>
              UDP <code>${esc(health.listen_udp || "—")}</code>
              · Kafka ${
                health.kafka_enabled
                  ? `<span class="badge on">已启用</span> <code>${esc(health.kafka_topic || "")}</code>`
                  : `<span class="badge off">未配置 brokers</span>`
              }
            </p>
            <div class="stat-row" style="display:flex;flex-wrap:wrap;gap:12px;font-size:0.9rem">
              <span>接收 ${esc(stats.received ?? 0)}</span>
              <span>解析成功 ${esc(stats.parsed_ok ?? 0)}</span>
              <span>解析失败 ${esc(stats.parse_err ?? 0)}</span>
              <span>Kafka 成功 ${esc(stats.kafka_ok ?? 0)}</span>
              <span>Kafka 失败 ${esc(stats.kafka_err ?? 0)}</span>
              <span>试推送 ${esc(stats.simulated ?? 0)}</span>
            </div>
          </div>
          <div class="panel" style="max-width:900px;margin-bottom:16px">
            <h3 style="margin:0 0 0.75rem;font-size:1rem">试推送（模拟 Trap）</h3>
            <p class="hint" style="margin:0 0 12px">写出与 Kafka Ingress Generic 对齐的 JSON，用于联调（不依赖真实设备）。</p>
            <div class="field"><label>设备 IP</label>
              <input id="trap-sim-ip" type="text" value="10.0.0.1" ${canWrite ? "" : "disabled"} />
            </div>
            <div class="field"><label>Trap OID</label>
              <input id="trap-sim-oid" type="text" value="1.3.6.1.6.3.1.1.5.3" ${canWrite ? "" : "disabled"} />
            </div>
            <div class="field"><label>告警名（可选）</label>
              <input id="trap-sim-name" type="text" placeholder="linkDown" ${canWrite ? "" : "disabled"} />
            </div>
            <div class="field"><label>级别</label>
              <select id="trap-sim-sev" ${canWrite ? "" : "disabled"}>
                ${severityOptions("warning")}
              </select>
            </div>
            <label class="check-row" style="margin:8px 0 14px">
              <input type="checkbox" id="trap-sim-dry" ${canWrite ? "" : "disabled"} />
              <span>仅预览 JSON（不写 Kafka）</span>
            </label>
            ${
              canWrite
                ? `<button class="primary" id="btn-trap-sim">试推送</button>`
                : `<p class="hint">需要 <code>trap:write</code> 才能试推送。</p>`
            }
            <pre id="trap-sim-out" class="mono" style="margin-top:12px;white-space:pre-wrap;font-size:0.8rem;max-height:240px;overflow:auto"></pre>
          </div>
          <div class="panel" style="max-width:900px">
            <h3 style="margin:0 0 0.75rem;font-size:1rem">最近事件</h3>
            <p class="hint" style="margin:0 0 12px">点击「详情」查看完整 OID / 变量名 / Varbind。</p>
            ${
              items.length
                ? `<table class="data"><thead><tr><th>时间</th><th>IP</th><th>OID</th><th>名称</th><th>Kafka</th><th></th></tr></thead>
                  <tbody>${items
                    .map(
                      (it, i) => `<tr>
                    <td>${esc(fmtTime(it.at))}</td>
                    <td class="mono">${esc(it.peer)}</td>
                    <td class="mono" title="${esc(it.trap_oid)}">${esc(
                        it.trap_oid && it.trap_oid.length > 36
                          ? it.trap_oid.slice(0, 36) + "…"
                          : it.trap_oid
                      )}</td>
                    <td>${esc(it.alertname)}</td>
                    <td>${it.kafka ? "✓" : "—"}</td>
                    <td class="actions"><button type="button" data-trap-detail="${i}">详情</button></td>
                  </tr>`
                    )
                    .join("")}</tbody></table>`
                : `<div class="empty">尚无 Trap / 试推送记录。</div>`
            }
          </div>`;

        root.querySelectorAll("[data-trap-detail]").forEach((b) => {
          b.onclick = () => showTrapRecentDetail(items[Number(b.dataset.trapDetail)]);
        });

        const btn = document.getElementById("btn-trap-sim");
        if (btn) {
          btn.onclick = async () => {
            try {
              const body = {
                ip: document.getElementById("trap-sim-ip").value.trim(),
                trap_oid: document.getElementById("trap-sim-oid").value.trim(),
                alertname: document.getElementById("trap-sim-name").value.trim() || null,
                severity: document.getElementById("trap-sim-sev").value,
                dry_run: document.getElementById("trap-sim-dry").checked,
              };
              const r = await api("/trap-api/api/simulate", {
                method: "POST",
                body: JSON.stringify(body),
              });
              document.getElementById("trap-sim-out").textContent = JSON.stringify(
                r.alert || r,
                null,
                2
              );
              toast(r.kafka ? "已写入 Kafka" : "已生成预览");
              await paint();
            } catch (e) {
              toast(e.message || String(e), true);
            }
          };
        }
      };
      document.getElementById("btn-trap-refresh").onclick = () => paint();
      await paint();
    },

    async mib(root) {
      const canWrite = can("trap:write");
      setActions(`
        <button class="ghost" id="btn-mib-reload">重新加载</button>
        <button class="ghost" id="btn-mib-export-all">导出全部策略</button>
        ${
          canWrite
            ? `<button class="primary" id="btn-mib-upload">上传 MIB</button>
               <input type="file" id="mib-file" accept=".mib,.txt,.my,.smi" hidden multiple />`
            : ""
        }
      `);

      state.mibSel = state.mibSel || null;
      state.mibTree = state.mibTree || {};
      state.mibExpanded = state.mibExpanded || {};
      state.mibQ = state.mibQ || "";
      state.mibTrapQ = state.mibTrapQ || "";
      let mibListCache = null;
      let mibNotifCache = {};

      const invalidateMibCache = () => {
        mibListCache = null;
        mibNotifCache = {};
      };

      const downloadPolicies = async (path) => {
        const t = token();
        const res = await fetch(path, {
          headers: t ? { Authorization: `Bearer ${t}` } : {},
        });
        if (!res.ok) {
          const text = await res.text();
          let msg = text;
          try {
            msg = JSON.parse(text).error || text;
          } catch (_) {}
          throw new Error(msg || res.statusText);
        }
        const blob = await res.blob();
        const cd = res.headers.get("Content-Disposition") || "";
        const m = /filename="?([^"]+)"?/.exec(cd);
        const name = (m && m[1]) || "trap-policies.json";
        const a = document.createElement("a");
        a.href = URL.createObjectURL(blob);
        a.download = name;
        a.click();
        URL.revokeObjectURL(a.href);
      };

      const uploadFiles = async (files) => {
        for (const file of files) {
          const buf = await file.arrayBuffer();
          const t = token();
          const res = await fetch(
            `/trap-api/api/mibs?filename=${encodeURIComponent(file.name)}`,
            {
              method: "POST",
              headers: {
                "Content-Type": "application/octet-stream",
                ...(t ? { Authorization: `Bearer ${t}` } : {}),
              },
              body: buf,
            }
          );
          const text = await res.text();
          let data = null;
          try {
            data = text ? JSON.parse(text) : null;
          } catch {
            data = { error: text };
          }
          if (!res.ok) throw new Error((data && data.error) || text || res.statusText);
          toast(`已上传 ${data.item?.module_name || file.name}`);
        }
      };

      const loadChildren = async (moduleId, oid) => {
        const key = `${moduleId}|${oid || ""}`;
        const q = oid ? `?oid=${encodeURIComponent(oid)}` : "";
        const data = await api(`/trap-api/api/mibs/${encodeURIComponent(moduleId)}/children${q}`);
        state.mibTree[key] = data.children || [];
        return state.mibTree[key];
      };

      const renderTreeNodes = (moduleId, parentOid, depth) => {
        const key = `${moduleId}|${parentOid || ""}`;
        const nodes = state.mibTree[key] || [];
        return nodes
          .map((n) => {
            const expKey = `${moduleId}|${n.oid}`;
            const expanded = !!state.mibExpanded[expKey];
            const kids = expanded ? renderTreeNodes(moduleId, n.oid, depth + 1) : "";
            const caret = n.has_children ? (expanded ? "▾" : "▸") : "·";
            return `<div class="mib-node" style="padding-left:${depth * 14}px">
              <button type="button" class="mib-node-btn ${
                state.mibFocusOid === n.oid ? "active" : ""
              }" data-mib-oid="${esc(n.oid)}" data-mib-expand="${n.has_children ? "1" : "0"}">
                <span class="mib-caret">${caret}</span>
                <span class="mib-name">${esc(n.name)}${
                  n.is_notification ? ' <span class="badge on">Trap</span>' : ""
                }</span>
                <span class="mib-oid mono">${esc(n.oid)}</span>
              </button>
              ${kids}
            </div>`;
          })
          .join("");
      };

      const filterMibModules = (items, qRaw) => {
        const q = String(qRaw || "")
          .trim()
          .toLowerCase();
        if (!q) return items;
        return items.filter((it) => {
          const blob = [it.module_name, it.id, it.filename, it.module_oid, it.error]
            .filter(Boolean)
            .join(" ")
            .toLowerCase();
          return blob.includes(q);
        });
      };

      const filterTrapRows = (rows, qRaw) => {
        const q = String(qRaw || "")
          .trim()
          .toLowerCase();
        if (!q) return rows;
        return rows.filter((r) => {
          const blob = [r.name, r.trap_oid, r.severity, (r.objects || []).join(" "), r.module]
            .filter(Boolean)
            .join(" ")
            .toLowerCase();
          return blob.includes(q);
        });
      };

      const paint = async (opts = {}) => {
        const reuse = !!opts.reuse;
        let list = mibListCache;
        if (!reuse || !list) {
          try {
            list = await api("/trap-api/api/mibs");
            mibListCache = list;
          } catch (e) {
            root.innerHTML = `<div class="panel"><p class="hint">Trap 服务未连通：${esc(
              e.message
            )}</p></div>`;
            return;
          }
        }
        const items = list.items || [];
        const filteredMods = filterMibModules(items, state.mibQ);
        if (!state.mibSel && items.length) state.mibSel = items[0].id;
        if (state.mibSel && !items.find((x) => x.id === state.mibSel)) {
          state.mibSel = items[0]?.id || null;
          state.mibTree = {};
          state.mibExpanded = {};
        }

        const focusId = document.activeElement?.id;
        const focusPos =
          focusId === "mib-q" || focusId === "mib-trap-q"
            ? document.activeElement.selectionStart
            : null;

        let detailHtml = `<div class="empty">选择左侧模块后浏览 OID 树</div>`;
        let notifHtml = "";
        if (state.mibSel) {
          const rootKey = `${state.mibSel}|`;
          if (!state.mibTree[rootKey]) {
            try {
              await loadChildren(state.mibSel, "");
            } catch (e) {
              detailHtml = `<div class="empty">${esc(e.message)}</div>`;
            }
          }
          const treeHtml = renderTreeNodes(state.mibSel, "", 0);
          let nodeInfo = `<p class="hint">点击节点查看详情；标有 Trap 的为 NOTIFICATION / TRAP-TYPE。</p>`;
          if (state.mibFocusOid) {
            try {
              const nd = await api(
                `/trap-api/api/mibs/${encodeURIComponent(state.mibSel)}/node?oid=${encodeURIComponent(
                  state.mibFocusOid
                )}`
              );
              nodeInfo = `
                <div class="field"><label>名称</label><div><strong>${esc(nd.name)}</strong>
                  ${nd.is_notification ? '<span class="badge on">Trap</span>' : ""}</div></div>
                <div class="field"><label>OID</label><div class="mono">${esc(nd.oid)}</div></div>
                <div class="field"><label>类型</label><div>${esc(nd.kind)} · ${esc(nd.status)}</div></div>
                <div class="field"><label>描述</label><div class="summary-box">${esc(
                  nd.description || "—"
                )}</div></div>
                ${
                  nd.objects && nd.objects.length
                    ? `<div class="field"><label>OBJECTS</label><div class="mono">${esc(
                        nd.objects.join(", ")
                      )}</div></div>`
                    : ""
                }`;
            } catch (e) {
              nodeInfo = `<p class="hint">${esc(e.message)}</p>`;
            }
          }
          detailHtml = `
            <div class="mib-browser">
              <div class="mib-tree">${treeHtml || `<div class="empty">无 OID 节点</div>`}</div>
              <div class="mib-detail">${nodeInfo}</div>
            </div>`;

          try {
            const cached = mibNotifCache[state.mibSel];
            let rows = cached;
            if (!rows) {
              const n = await api(
                `/trap-api/api/mibs/${encodeURIComponent(state.mibSel)}/notifications`
              );
              rows = n.items || [];
              mibNotifCache[state.mibSel] = rows;
            }
            const shown = filterTrapRows(rows, state.mibTrapQ);
            notifHtml = `
              <div class="panel" style="margin-top:16px">
                <div style="display:flex;justify-content:space-between;align-items:center;gap:12px;margin-bottom:8px;flex-wrap:wrap">
                  <h3 style="margin:0;font-size:1rem">可导出的 Trap（${
                    state.mibTrapQ.trim()
                      ? `${shown.length} / ${rows.length}`
                      : rows.length
                  }）</h3>
                  <div style="display:flex;gap:8px;flex-wrap:wrap">
                    <button type="button" class="ghost" id="btn-mib-export-one">下载 Excel</button>
                    ${
                      canWrite && rows.length
                        ? `<button type="button" class="primary" id="btn-mib-import-pol">导入到策略</button>`
                        : ""
                    }
                  </div>
                </div>
                ${
                  rows.length
                    ? `<div class="alert-filters" style="margin-bottom:10px">
                        <input id="mib-trap-q" placeholder="搜索 Trap 名称、OID、OBJECTS…" value="${esc(
                          state.mibTrapQ || ""
                        )}" />
                      </div>
                      ${
                        shown.length
                          ? `<div class="alert-table-scroll"><table class="data"><thead><tr>
                        <th>名称</th><th>OID</th><th>级别</th><th>OBJECTS</th></tr></thead>
                      <tbody>${shown
                        .map((r) => {
                          const objs = (r.objects || []).join(", ");
                          return `<tr>
                          <td>${esc(r.name)}</td>
                          <td class="mono">${esc(r.trap_oid)}</td>
                          <td>${esc(r.severity)}</td>
                          <td class="mono"><span class="mib-objects" title="${esc(
                            objs
                          )}">${esc(objs || "—")}</span></td>
                        </tr>`;
                        })
                        .join("")}</tbody></table></div>`
                          : `<div class="empty">没有符合筛选条件的 Trap。</div>`
                      }`
                    : `<div class="empty">该 MIB 未定义 NOTIFICATION-TYPE / TRAP-TYPE。</div>`
                }
              </div>`;
          } catch (e) {
            notifHtml = `<div class="panel" style="margin-top:16px"><p class="hint">${esc(
              e.message
            )}</p></div>`;
          }
        }

        root.innerHTML = `
          ${
            list.load_error
              ? `<div class="panel" style="margin-bottom:12px"><p class="hint">解析警告：${esc(
                  list.load_error
                )}</p></div>`
              : ""
          }
          <div class="mib-layout">
            <div class="panel mib-side">
              <h3 style="margin:0 0 8px;font-size:1rem">已上传模块</h3>
              <p class="hint" style="margin:0 0 10px">目录 <code>${esc(list.mib_dir || "")}</code>${
                list.backend ? ` · <code>${esc(list.backend)}</code>` : ""
              }
                · 显示 ${esc(filteredMods.length)} / ${esc(items.length)}</p>
              ${
                items.length
                  ? `<div class="alert-filters" style="margin-bottom:10px">
                      <input id="mib-q" placeholder="搜索模块名、文件名、OID…" value="${esc(
                        state.mibQ || ""
                      )}" />
                    </div>
                    ${
                      filteredMods.length
                        ? `<ul class="mib-mod-list">${filteredMods
                            .map(
                              (it) => `<li>
                      <div class="mib-mod ${
                        state.mibSel === it.id ? "active" : ""
                      }" role="button" tabindex="0" data-mib-mod="${esc(it.id)}">
                        <span class="mib-mod-title">${esc(it.module_name)}</span>
                        <span class="mib-mod-meta">${it.parse_ok ? "已解析" : "解析失败"} · Trap ${esc(
                          it.notification_count
                        )} · 节点 ${esc(it.node_count)}</span>
                      </div>
                      ${
                        canWrite
                          ? `<button type="button" class="danger ghost mib-del" data-mib-del="${esc(
                              it.id
                            )}" title="删除">×</button>`
                          : ""
                      }
                    </li>`
                            )
                            .join("")}</ul>`
                        : `<div class="empty">没有符合筛选条件的模块。</div>`
                    }`
                  : `<div class="empty">尚未上传 MIB。可先用仓库 <code>crates/eventide-trap/fixtures/EXAMPLE-FULL-MIB.txt</code> 试传。</div>`
              }
            </div>
            <div class="mib-main">
              <div class="panel">${detailHtml}</div>
              ${notifHtml}
            </div>
          </div>`;

        const bindSearch = (id, key) => {
          const el = root.querySelector(`#${id}`);
          if (!el) return;
          let t = null;
          el.oninput = () => {
            clearTimeout(t);
            t = setTimeout(() => {
              state[key] = el.value || "";
              paint({ reuse: true });
            }, 200);
          };
          if (focusId === id) {
            el.focus();
            if (focusPos != null) {
              try {
                el.setSelectionRange(focusPos, focusPos);
              } catch (_) {}
            }
          }
        };
        bindSearch("mib-q", "mibQ");
        bindSearch("mib-trap-q", "mibTrapQ");

        root.querySelectorAll("[data-mib-mod]").forEach((b) => {
          const select = async () => {
            state.mibSel = b.dataset.mibMod;
            state.mibFocusOid = null;
            state.mibTree = {};
            state.mibExpanded = {};
            state.mibTrapQ = "";
            await paint({ reuse: true });
          };
          b.onclick = select;
          b.onkeydown = (ev) => {
            if (ev.key === "Enter" || ev.key === " ") {
              ev.preventDefault();
              select();
            }
          };
        });
        root.querySelectorAll("[data-mib-del]").forEach((b) => {
          b.onclick = async (ev) => {
            ev.stopPropagation();
            if (!confirm(`删除模块 ${b.dataset.mibDel}？`)) return;
            try {
              await api(`/trap-api/api/mibs/${encodeURIComponent(b.dataset.mibDel)}`, {
                method: "DELETE",
              });
              toast("已删除");
              state.mibSel = null;
              invalidateMibCache();
              await paint();
            } catch (e) {
              toast(e.message, true);
            }
          };
        });
        root.querySelectorAll("[data-mib-oid]").forEach((b) => {
          b.onclick = async () => {
            const oid = b.dataset.mibOid;
            const canExp = b.dataset.mibExpand === "1";
            state.mibFocusOid = oid;
            if (canExp && state.mibSel) {
              const expKey = `${state.mibSel}|${oid}`;
              if (!state.mibExpanded[expKey]) {
                state.mibExpanded[expKey] = true;
                try {
                  await loadChildren(state.mibSel, oid);
                } catch (e) {
                  toast(e.message, true);
                }
              } else {
                state.mibExpanded[expKey] = false;
              }
            }
            await paint({ reuse: true });
          };
        });
        const expOne = document.getElementById("btn-mib-export-one");
        if (expOne && state.mibSel) {
          expOne.onclick = async () => {
            try {
              await downloadPolicies(
                `/trap-api/api/mibs/${encodeURIComponent(state.mibSel)}/export-policies`
              );
              toast("已导出");
            } catch (e) {
              toast(e.message, true);
            }
          };
        }
        const impPol = document.getElementById("btn-mib-import-pol");
        if (impPol && state.mibSel) {
          impPol.onclick = async () => {
            try {
              const mode = await askPolicyImportMode("从 MIB 导入到策略");
              if (mode === null) return;
              const r = await api(
                `/trap-api/api/mibs/${encodeURIComponent(state.mibSel)}/apply-policies?mode=${mode}`,
                { method: "POST" }
              );
              toast(
                `${mode === "skip" ? "已忽略重复" : "已合并"}：新增 ${
                  r.result?.created ?? 0
                }，更新 ${r.result?.updated ?? 0}，跳过 ${r.result?.skipped ?? 0}`
              );
            } catch (e) {
              toast(e.message, true);
            }
          };
        }
      };

      document.getElementById("btn-mib-reload").onclick = async () => {
        try {
          await api("/trap-api/api/mibs/reload", { method: "POST" });
          state.mibTree = {};
          invalidateMibCache();
          toast("已重新加载");
          await paint();
        } catch (e) {
          toast(e.message, true);
        }
      };
      document.getElementById("btn-mib-export-all").onclick = async () => {
        try {
          await downloadPolicies("/trap-api/api/mibs/export-policies");
          toast("已导出全部策略");
        } catch (e) {
          toast(e.message, true);
        }
      };
      const fileInput = document.getElementById("mib-file");
      const uploadBtn = document.getElementById("btn-mib-upload");
      if (uploadBtn && fileInput) {
        uploadBtn.onclick = () => fileInput.click();
        fileInput.onchange = async () => {
          try {
            await uploadFiles([...fileInput.files]);
            fileInput.value = "";
            state.mibTree = {};
            invalidateMibCache();
            await paint();
          } catch (e) {
            toast(e.message, true);
          }
        };
      }
      await paint();
    },

    async policies(root) {
      const canWrite = can("trap:write");
      setActions(`
        <button class="ghost" id="btn-pol-refresh">刷新</button>
        <button class="ghost" id="btn-pol-export">导出</button>
        ${
          canWrite
            ? `<button class="ghost" id="btn-pol-import">导入 Excel</button>
               <input type="file" id="pol-file" accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" hidden />
               <button class="primary" id="btn-pol-add">新建策略</button>`
            : ""
        }
      `);

      const downloadPolicies = async (path) => {
        const t = token();
        const res = await fetch(path, {
          headers: t ? { Authorization: `Bearer ${t}` } : {},
        });
        if (!res.ok) {
          const text = await res.text();
          let msg = text;
          try {
            msg = JSON.parse(text).error || text;
          } catch (_) {}
          throw new Error(msg || res.statusText);
        }
        const blob = await res.blob();
        const cd = res.headers.get("Content-Disposition") || "";
        const m = /filename="?([^"]+)"?/.exec(cd);
        const name = (m && m[1]) || "trap-policies.json";
        const a = document.createElement("a");
        a.href = URL.createObjectURL(blob);
        a.download = name;
        a.click();
        URL.revokeObjectURL(a.href);
      };

      const editPolicy = (row) => {
        const p = row || {
          id: "",
          name: "",
          trap_oid: "",
          match_mode: "exact",
          severity: "warning",
          enabled: true,
          summary_template: "${alertname}: ${ip}",
          description: "",
          objects: [],
          object_oids: {},
          keywords: [],
          module: "",
          status: "",
          resolve_oid: "",
          resolve_values: [],
          fingerprint_oids: [],
          severity_oid: "",
          severity_map: {},
        };
        const resolveVals = Array.isArray(p.resolve_values)
          ? p.resolve_values.join(", ")
          : "";
        const fpOids = Array.isArray(p.fingerprint_oids)
          ? p.fingerprint_oids.join(", ")
          : "";
        const sevMapText = Object.entries(p.severity_map || {})
          .map(([k, v]) => `${k}=${v}`)
          .join(";");
        const objNames = [
          ...new Set([
            ...Object.keys(p.object_oids || {}),
            ...(Array.isArray(p.objects) ? p.objects : []),
          ]),
        ].filter(Boolean);
        const varsHtml = [
          "alertname",
          "ip",
          "trap_oid",
          ...objNames,
        ]
          .map((n) => `<code class="pol-var" title="\${${esc(n)}}">\${${esc(n)}}</code>`)
          .join("");
        openModal(
          `
          <div class="modal-head">
            <h3>${p.id ? "编辑策略" : "新建策略"}</h3>
            <p class="desc">同一 Trap OID 一条策略；默认告警，恢复条件命中则为恢复。</p>
          </div>
          <form id="pol-form" class="modal-body pol-form">
            <section class="enrich-section">
              <h4>基本信息</h4>
              <div class="row">
                <div class="field" style="flex:1.4"><label>名称</label>
                  <input name="name" required value="${esc(p.name)}" />
                </div>
                <div class="field" style="flex:1"><label>模块</label>
                  <input name="module" placeholder="可选" value="${esc(p.module || "")}" />
                </div>
              </div>
              <div class="field"><label>Trap OID</label>
                <input name="trap_oid" required class="mono" value="${esc(p.trap_oid)}" />
              </div>
              <div class="row">
                <div class="field"><label>匹配方式</label>
                  <select name="match_mode">
                    <option value="exact" ${p.match_mode === "exact" ? "selected" : ""}>精确</option>
                    <option value="prefix" ${p.match_mode === "prefix" ? "selected" : ""}>前缀</option>
                  </select>
                </div>
                <div class="field"><label>默认级别</label>
                  <select name="severity">
                    ${severityOptions(p.severity || "warning")}
                  </select>
                </div>
              </div>
              <label class="check-row" style="margin-bottom:12px">
                <input type="checkbox" name="enabled" ${
                  p.enabled !== false ? "checked" : ""
                } /><span>启用此策略</span>
              </label>
            </section>

            <section class="enrich-section">
              <h4>告警与恢复</h4>
              <div class="row">
                <div class="field" style="flex:1.5"><label>恢复 OID</label>
                  <input name="resolve_oid" class="mono" placeholder="例：…10.3.1.1.11" value="${esc(
                    p.resolve_oid || ""
                  )}" />
                </div>
                <div class="field"><label>恢复值</label>
                  <input name="resolve_values" placeholder="逗号分隔，如 2" value="${esc(
                    resolveVals
                  )}" />
                </div>
              </div>
              <p class="field-hint">该 OID 的 varbind 值命中任一项时标记为恢复；可填 OBJECTS 名或数字 OID。</p>
              <div class="field"><label>指纹 OID</label>
                <input name="fingerprint_oids" class="mono" placeholder="逗号分隔，例：…10.3.1.1.9" value="${esc(
                  fpOids
                )}" />
              </div>
              <p class="field-hint">取这些 varbind 的<strong>值</strong>关联告警与恢复（如流水号）。</p>
            </section>

            <section class="enrich-section">
              <h4>动态级别</h4>
              <div class="row">
                <div class="field"><label>级别 OID</label>
                  <input name="severity_oid" class="mono" placeholder="例：…10.3.1.1.6" value="${esc(
                    p.severity_oid || ""
                  )}" />
                </div>
                <div class="field" style="flex:1.4"><label>级别映射</label>
                  <input name="severity_map" class="mono" placeholder="1=disaster;2=high;3=average;4=warning" value="${esc(
                    sevMapText
                  )}" />
                </div>
              </div>
              <p class="field-hint">原值=平台级别，分号分隔；未命中映射时用上方默认级别。</p>
            </section>

            <section class="enrich-section">
              <h4>告警内容</h4>
              <div class="field"><label>摘要模板</label>
                <textarea name="summary_template" class="mono pol-summary" rows="4">${esc(
                  p.summary_template || ""
                )}</textarea>
              </div>
              <p class="field-hint" style="margin-top:-8px">点击下方变量可插入到光标处</p>
              <div class="pol-vars">${varsHtml}</div>
              <div class="field"><label>描述</label>
                <textarea name="description" rows="2" placeholder="可选">${esc(
                  p.description || ""
                )}</textarea>
              </div>
            </section>

            <input type="hidden" name="id" value="${esc(p.id || "")}" />
            <input type="hidden" name="objects_json" value="${esc(
              JSON.stringify(p.objects || [])
            )}" />
            <input type="hidden" name="object_oids_json" value="${esc(
              JSON.stringify(p.object_oids || {})
            )}" />
            <input type="hidden" name="keywords_json" value="${esc(
              JSON.stringify(p.keywords || [])
            )}" />
            <input type="hidden" name="status" value="${esc(p.status || "")}" />
          </form>
          <div class="modal-actions">
            <button type="button" class="ghost" id="pol-cancel">取消</button>
            <button type="button" class="primary" id="pol-save">保存</button>
          </div>
        `,
          { xl: true }
        );
        document.getElementById("pol-cancel").onclick = () => closeModal();
        document.querySelectorAll("#pol-form .pol-var").forEach((el) => {
          el.onclick = () => {
            const ta = document.querySelector(
              '#pol-form textarea[name="summary_template"]'
            );
            if (!ta) return;
            const token = el.textContent || "";
            const start = ta.selectionStart ?? ta.value.length;
            const end = ta.selectionEnd ?? start;
            ta.value = ta.value.slice(0, start) + token + ta.value.slice(end);
            ta.focus();
            const pos = start + token.length;
            try {
              ta.setSelectionRange(pos, pos);
            } catch (_) {}
          };
        });
        document.getElementById("pol-save").onclick = async () => {
          const form = document.getElementById("pol-form");
          const fd = new FormData(form);
          let objects = [],
            object_oids = {},
            keywords = [];
          try {
            objects = JSON.parse(String(fd.get("objects_json") || "[]"));
            object_oids = JSON.parse(String(fd.get("object_oids_json") || "{}"));
            keywords = JSON.parse(String(fd.get("keywords_json") || "[]"));
          } catch (_) {}
          const resolve_values = String(fd.get("resolve_values") || "")
            .split(/[,，;；]+/)
            .map((s) => s.trim())
            .filter(Boolean);
          const fingerprint_oids = String(fd.get("fingerprint_oids") || "")
            .split(/[,，;；]+/)
            .map((s) => s.trim())
            .filter(Boolean);
          const severity_map = {};
          String(fd.get("severity_map") || "")
            .split(/[;；]+/)
            .map((s) => s.trim())
            .filter(Boolean)
            .forEach((pair) => {
              const i = pair.indexOf("=");
              if (i > 0) {
                const k = pair.slice(0, i).trim();
                const v = pair.slice(i + 1).trim();
                if (k && v) severity_map[k] = v;
              }
            });
          const body = {
            id: String(fd.get("id") || ""),
            name: String(fd.get("name") || "").trim(),
            trap_oid: String(fd.get("trap_oid") || "").trim(),
            match_mode: String(fd.get("match_mode") || "exact"),
            severity: String(fd.get("severity") || "warning"),
            enabled: !!form.querySelector('[name="enabled"]')?.checked,
            summary_template: String(fd.get("summary_template") || ""),
            description: String(fd.get("description") || ""),
            module: String(fd.get("module") || ""),
            status: String(fd.get("status") || ""),
            resolve_oid: String(fd.get("resolve_oid") || "").trim(),
            resolve_values,
            fingerprint_oids,
            severity_oid: String(fd.get("severity_oid") || "").trim(),
            severity_map,
            objects,
            object_oids,
            keywords,
            updated_at: "",
          };
          try {
            if (body.id) {
              await api(`/trap-api/api/policies/${encodeURIComponent(body.id)}`, {
                method: "PUT",
                body: JSON.stringify(body),
              });
            } else {
              await api("/trap-api/api/policies", {
                method: "POST",
                body: JSON.stringify(body),
              });
            }
            toast("已保存");
            closeModal();
            await paint();
          } catch (e) {
            toast(e.message, true);
          }
        };
      };

      let polCache = { items: [], path: "" };

      const renderPolicies = () => {
        const all = polCache.items || [];
        const f = state.policyFilters || {
          q: "",
          enabled: "",
          module: "",
          resolve: "",
        };
        const q = (f.q || "").trim().toLowerCase();
        const modules = [
          ...new Set(all.map((p) => p.module).filter((m) => m && String(m).trim())),
        ].sort();

        const filtered = all.filter((p) => {
          if (f.enabled === "1" && !p.enabled) return false;
          if (f.enabled === "0" && p.enabled) return false;
          if (f.module && (p.module || "") !== f.module) return false;
          if (f.resolve === "1") {
            const has =
              (p.resolve_oid && String(p.resolve_oid).trim()) ||
              (p.resolve_values && p.resolve_values.length) ||
              p.status === "resolved";
            if (!has) return false;
          }
          if (f.resolve === "0") {
            const has =
              (p.resolve_oid && String(p.resolve_oid).trim()) ||
              (p.resolve_values && p.resolve_values.length) ||
              p.status === "resolved";
            if (has) return false;
          }
          if (q) {
            const blob = [
              p.name,
              p.trap_oid,
              p.module,
              p.summary_template,
              p.description,
              p.resolve_oid,
              (p.resolve_values || []).join(" "),
              (p.fingerprint_oids || []).join(" "),
              p.severity_oid,
              p.severity,
            ]
              .join(" ")
              .toLowerCase();
            if (!blob.includes(q)) return false;
          }
          return true;
        });

        const focusId = document.activeElement?.id;
        const focusPos =
          focusId === "pol-q" ? document.activeElement.selectionStart : null;

        root.innerHTML = `
          <div class="panel" style="margin-bottom:12px">
            <p class="hint" style="margin:0 0 10px">
              策略存储 <code>${esc(polCache.path || "")}</code> ·
              显示 ${esc(filtered.length)} / ${esc(all.length)} 条。
            </p>
            <div class="alert-filters">
              <input id="pol-q" placeholder="搜索名称、OID、模块、摘要…" value="${esc(f.q || "")}" />
              <select id="pol-enabled">
                <option value="" ${!f.enabled ? "selected" : ""}>启用·全部</option>
                <option value="1" ${f.enabled === "1" ? "selected" : ""}>已启用</option>
                <option value="0" ${f.enabled === "0" ? "selected" : ""}>已禁用</option>
              </select>
              <select id="pol-module">
                <option value="" ${!f.module ? "selected" : ""}>模块·全部</option>
                ${modules
                  .map(
                    (m) =>
                      `<option value="${esc(m)}" ${f.module === m ? "selected" : ""}>${esc(
                        m
                      )}</option>`
                  )
                  .join("")}
              </select>
              <select id="pol-resolve">
                <option value="" ${!f.resolve ? "selected" : ""}>恢复条件·全部</option>
                <option value="1" ${f.resolve === "1" ? "selected" : ""}>已配置恢复</option>
                <option value="0" ${f.resolve === "0" ? "selected" : ""}>未配置恢复</option>
              </select>
            </div>
          </div>
          <div class="panel">
            ${
              filtered.length
                ? `<div class="alert-table-scroll"><table class="data"><thead><tr>
                    <th>启用</th><th>名称</th><th>OID</th><th>恢复</th><th>指纹OID</th><th>级别映射</th><th>模块</th><th>摘要</th><th></th>
                  </tr></thead><tbody>${filtered
                    .map(
                      (p) => `<tr>
                      <td>${p.enabled ? "✓" : "—"}</td>
                      <td>${esc(p.name)}</td>
                      <td class="mono">${esc(p.trap_oid)}</td>
                      <td class="mono"><span class="mib-objects" title="${esc(
                        `${p.resolve_oid || ""} = ${(p.resolve_values || []).join(",")}`
                      )}">${
                        p.resolve_oid
                          ? esc(`${p.resolve_oid}→${(p.resolve_values || []).join(",")}`)
                          : p.status === "resolved"
                            ? "(始终恢复)"
                            : "—"
                      }</span></td>
                      <td class="mono"><span class="mib-objects" title="${esc(
                        (p.fingerprint_oids || []).join(", ")
                      )}">${esc((p.fingerprint_oids || []).join(", ") || "—")}</span></td>
                      <td class="mono"><span class="mib-objects" title="${esc(
                        p.severity_oid
                          ? `${p.severity_oid}: ${Object.entries(p.severity_map || {})
                              .map(([k, v]) => k + "=" + v)
                              .join(";")}`
                          : ""
                      )}">${
                        p.severity_oid
                          ? esc(p.severity_oid)
                          : esc(p.severity || "—")
                      }</span></td>
                      <td>${esc(p.module || "—")}</td>
                      <td class="mono"><span class="mib-objects" title="${esc(
                        p.summary_template || ""
                      )}">${esc(p.summary_template || "—")}</span></td>
                      <td class="actions">
                        ${
                          canWrite
                            ? `<button type="button" data-pol-edit="${esc(p.id)}">编辑</button>
                               <button type="button" class="danger" data-pol-del="${esc(
                                 p.id
                               )}">删除</button>`
                            : ""
                        }
                      </td>
                    </tr>`
                    )
                    .join("")}</tbody></table></div>`
                : `<div class="empty">${
                    all.length
                      ? "没有符合筛选条件的策略。"
                      : "暂无策略。请到「MIB 库」点「导入到策略」，或在此「导入 Excel」 /「新建策略」。"
                  }</div>`
            }
          </div>`;

        const readFilters = () => {
          state.policyFilters = {
            q: root.querySelector("#pol-q")?.value || "",
            enabled: root.querySelector("#pol-enabled")?.value || "",
            module: root.querySelector("#pol-module")?.value || "",
            resolve: root.querySelector("#pol-resolve")?.value || "",
          };
        };
        const qEl = root.querySelector("#pol-q");
        if (qEl) {
          let t = null;
          qEl.oninput = () => {
            clearTimeout(t);
            t = setTimeout(() => {
              readFilters();
              renderPolicies();
            }, 200);
          };
          if (focusId === "pol-q") {
            qEl.focus();
            if (focusPos != null) {
              try {
                qEl.setSelectionRange(focusPos, focusPos);
              } catch (_) {}
            }
          }
        }
        root.querySelector("#pol-enabled")?.addEventListener("change", () => {
          readFilters();
          renderPolicies();
        });
        root.querySelector("#pol-module")?.addEventListener("change", () => {
          readFilters();
          renderPolicies();
        });
        root.querySelector("#pol-resolve")?.addEventListener("change", () => {
          readFilters();
          renderPolicies();
        });

        const byId = Object.fromEntries(all.map((p) => [p.id, p]));
        root.querySelectorAll("[data-pol-edit]").forEach((b) => {
          b.onclick = () => {
            const row = byId[b.dataset.polEdit];
            if (row) editPolicy(row);
          };
        });
        root.querySelectorAll("[data-pol-del]").forEach((b) => {
          b.onclick = async () => {
            if (!confirm("删除该策略？")) return;
            try {
              await api(`/trap-api/api/policies/${encodeURIComponent(b.dataset.polDel)}`, {
                method: "DELETE",
              });
              toast("已删除");
              await paint();
            } catch (e) {
              toast(e.message, true);
            }
          };
        });
      };

      const paint = async () => {
        try {
          const data = await api("/trap-api/api/policies");
          polCache = { items: data.items || [], path: data.path || "" };
          renderPolicies();
        } catch (e) {
          root.innerHTML = `<div class="panel"><p class="hint">Trap 服务未连通：${esc(
            e.message
          )}</p></div>`;
        }
      };

      document.getElementById("btn-pol-refresh").onclick = () => paint();
      document.getElementById("btn-pol-export").onclick = async () => {
        try {
          await downloadPolicies("/trap-api/api/policies/export");
          toast("已导出");
        } catch (e) {
          toast(e.message, true);
        }
      };
      const importBtn = document.getElementById("btn-pol-import");
      const fileInput = document.getElementById("pol-file");
      if (importBtn && fileInput) {
        importBtn.onclick = () => fileInput.click();
        fileInput.onchange = async () => {
          const file = fileInput.files?.[0];
          if (!file) return;
          try {
            const mode = await askPolicyImportMode("导入 Excel 策略");
            if (mode === null) {
              fileInput.value = "";
              return;
            }
            const buf = await file.arrayBuffer();
            const t = token();
            const res = await fetch(
              `/trap-api/api/policies/import?mode=${mode}`,
              {
                method: "POST",
                headers: {
                  "Content-Type":
                    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                  ...(t ? { Authorization: `Bearer ${t}` } : {}),
                },
                body: buf,
              }
            );
            const text = await res.text();
            let data = null;
            try {
              data = text ? JSON.parse(text) : null;
            } catch {
              data = { error: text };
            }
            if (!res.ok) throw new Error((data && data.error) || text || res.statusText);
            toast(
              `${mode === "skip" ? "已忽略重复" : "已合并"}：新增 ${
                data.result?.created ?? 0
              }，更新 ${data.result?.updated ?? 0}，跳过 ${data.result?.skipped ?? 0}`
            );
            fileInput.value = "";
            await paint();
          } catch (e) {
            toast(e.message, true);
            fileInput.value = "";
          }
        };
      }
      const addBtn = document.getElementById("btn-pol-add");
      if (addBtn) addBtn.onclick = () => editPolicy(null);
      await paint();
    },

    async settings(root) {
      const me = await api("/api/auth/me");
      state.me = me;
      applyNavPermissions();
      const perms = (me.permissions || []).join(", ") || "—";
      let hist = {
        write_to_es: false,
        search_store: "mysql",
        es_configured: false,
        es_url: "",
        es_index: "eventide-alerts",
        es_username: "",
        es_password_set: false,
      };
      try {
        hist = await api("/api/settings/alert-history");
      } catch (_) {}
      const canWriteSettings = can("settings:write");
      const dis = canWriteSettings ? "" : "disabled";
      root.innerHTML = `
        <div class="panel" style="max-width:720px;margin-bottom:16px">
          <h3 style="margin:0 0 0.75rem;font-size:1rem">外观主题</h3>
          <p class="hint" style="margin:0 0 12px">选择会写入本机偏好，登录页与侧栏也可切换。</p>
          <div id="settings-theme"></div>
        </div>
        <div class="panel" style="max-width:720px;margin-bottom:16px">
          <h3 style="margin:0 0 0.75rem;font-size:1rem">告警历史仓库</h3>
          <p class="hint" style="margin:0 0 12px">
            在此填写 Elasticsearch 地址并保存即可生效（也可在 <code>eventide.toml</code> 里预置）。
            状态：${
              hist.es_configured
                ? `<span class="badge on">已连接配置</span>`
                : `<span class="badge off">未配置地址</span>`
            }
          </p>
          <div class="field"><label>ES 地址</label>
            <input id="hist-es-url" type="text" placeholder="http://127.0.0.1:9200" value="${esc(
              hist.es_url || ""
            )}" ${dis} />
          </div>
          <div class="field"><label>Index</label>
            <input id="hist-es-index" type="text" placeholder="eventide-alerts" value="${esc(
              hist.es_index || "eventide-alerts"
            )}" ${dis} />
          </div>
          <div class="field"><label>用户名（可选）</label>
            <input id="hist-es-user" type="text" autocomplete="off" value="${esc(
              hist.es_username || ""
            )}" ${dis} />
          </div>
          <div class="field"><label>密码（可选）</label>
            <input id="hist-es-pass" type="password" autocomplete="new-password" placeholder="${
              hist.es_password_set ? "已保存，留空则不修改" : "无密码可留空"
            }" ${dis} />
          </div>
          <label class="check-row" style="margin:12px 0">
            <input type="checkbox" id="hist-write-es" ${hist.write_to_es ? "checked" : ""} ${dis} />
            <span>同步告警到历史事件（Elasticsearch）</span>
          </label>
          <div class="field" style="margin-top:8px">
            <label>默认列表范围</label>
            <select id="hist-search-store" ${dis}>
              <option value="mysql" ${hist.search_store === "mysql" ? "selected" : ""}>最近事件</option>
              <option value="es" ${hist.search_store === "es" ? "selected" : ""}>历史事件</option>
            </select>
          </div>
          ${
            canWriteSettings
              ? `<div style="margin-top:14px"><button class="primary" id="hist-save">保存</button></div>`
              : `<p class="hint">需要 <code>settings:write</code> 权限才能修改。</p>`
          }
        </div>
        <div class="panel" style="max-width:720px">
          <h3 style="margin:0 0 1rem;font-size:1rem">运行信息</h3>
          <div class="field"><label>当前用户</label><div>${esc(me.display_name || me.username)}（${esc(
        me.username
      )}）</div></div>
          <div class="field"><label>权限</label><div class="mono" style="font-size:12px;word-break:break-all">${esc(
            perms
          )}</div></div>
          <div class="field"><label>监听地址</label><div class="mono">${esc(me.listen)}</div></div>
          <div class="field"><label>数据库</label><div class="mono">${esc(me.database_path)}</div></div>
          <div class="field"><label>静态资源目录</label><div class="mono">${esc(me.static_dir)}</div></div>
          <div class="field"><label>调度周期</label><div>${me.scheduler_tick_seconds} 秒</div></div>
          <div class="field"><label>Token 有效期</label><div>${me.token_ttl_hours} 小时</div></div>
          <p style="color:var(--muted);font-size:0.88rem;margin:1rem 0 0">
            账号请在「用户管理」维护。JWT 密钥与有效期见 <code>eventide.toml</code> 的 <code>[auth]</code> 段（<code>jwt_secret</code> / <code>token_ttl_hours</code>）；首次空库会用其中的 username/password 种子管理员。
          </p>
        </div>`;
      bindThemeHost(document.getElementById("settings-theme"), "cards");
      const saveBtn = document.getElementById("hist-save");
      if (saveBtn) {
        saveBtn.onclick = async () => {
          try {
            const body = {
              write_to_es: !!document.getElementById("hist-write-es")?.checked,
              search_store: document.getElementById("hist-search-store")?.value || "mysql",
              es_url: (document.getElementById("hist-es-url")?.value || "").trim(),
              es_index: (document.getElementById("hist-es-index")?.value || "").trim(),
              es_username: (document.getElementById("hist-es-user")?.value || "").trim(),
              es_password: document.getElementById("hist-es-pass")?.value || "",
            };
            hist = await api("/api/settings/alert-history", {
              method: "PUT",
              body: JSON.stringify(body),
            });
            toast("告警历史设置已保存");
            renderPage();
          } catch (e) {
            toast(e.message, true);
          }
        };
      }
    },

    async users(root) {
      const canWrite = can("users:write");
      setActions(
        canWrite ? `<button class="primary" id="btn-add">新建用户</button>` : ""
      );
      if (canWrite) document.getElementById("btn-add").onclick = () => editUser();
      const [rows, roles, depts] = await Promise.all([
        api("/api/users"),
        api("/api/roles"),
        api("/api/departments"),
      ]);
      state.cache.roles = roles;
      state.cache.departments = depts;
      const roleMap = Object.fromEntries(roles.map((r) => [r.id, r.name]));
      const deptMap = Object.fromEntries(depts.map((d) => [d.id, d.name]));
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>用户名</th><th>显示名</th><th>部门</th><th>角色</th><th>状态</th><th></th></tr></thead>
            <tbody>${rows
              .map((u) => {
                const roleNames = (u.role_ids || [])
                  .map((id) => roleMap[id] || id.slice(0, 8))
                  .join("、") || "—";
                const dept = u.department_id ? deptMap[u.department_id] || "—" : "—";
                return `<tr>
              <td class="mono">${esc(u.username)}</td>
              <td>${esc(u.display_name || "—")}</td>
              <td>${esc(dept)}</td>
              <td>${esc(roleNames)}</td>
              <td><span class="badge ${u.enabled ? "on" : "off"}">${u.enabled ? "启用" : "停用"}</span></td>
              <td class="actions">${
                canWrite
                  ? `<button data-edit="${u.id}">编辑</button>
                <button data-pw="${u.id}">重置密码</button>
                <button class="danger" data-del="${u.id}">删除</button>`
                  : ""
              }</td></tr>`;
              })
              .join("")}</tbody></table>`
          : `<div class="empty">暂无用户。</div>`
      }</div>`;
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editUser(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-pw]").forEach((b) => {
        b.onclick = () => resetUserPassword(b.dataset.pw);
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该用户？")) return;
          try {
            await api(`/api/users/${b.dataset.del}`, { method: "DELETE" });
            toast("已删除");
            renderPage();
          } catch (e) {
            toast(e.message, true);
          }
        };
      });
    },

    async roles(root) {
      const canWrite = can("roles:write");
      setActions(canWrite ? `<button class="primary" id="btn-add">新建角色</button>` : "");
      if (canWrite) document.getElementById("btn-add").onclick = () => editRole();
      const [rows, catalog] = await Promise.all([api("/api/roles"), api("/api/permissions")]);
      state.cache.permCatalog = catalog;
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>名称</th><th>说明</th><th>权限</th><th></th></tr></thead>
            <tbody>${rows
              .map((r) => {
                const perms = (r.permissions || []).join(", ") || "—";
                return `<tr>
              <td>${esc(r.name)}${r.is_system ? ' <span class="badge on">系统</span>' : ""}</td>
              <td>${esc(r.description || "—")}</td>
              <td class="mono" style="font-size:12px;max-width:360px;word-break:break-all">${esc(
                perms
              )}</td>
              <td class="actions">${
                canWrite
                  ? `<button data-edit="${r.id}">编辑</button>
                ${
                  r.is_system
                    ? ""
                    : `<button class="danger" data-del="${r.id}">删除</button>`
                }`
                  : ""
              }</td></tr>`;
              })
              .join("")}</tbody></table>`
          : `<div class="empty">暂无角色。</div>`
      }</div>`;
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editRole(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该角色？")) return;
          try {
            await api(`/api/roles/${b.dataset.del}`, { method: "DELETE" });
            toast("已删除");
            renderPage();
          } catch (e) {
            toast(e.message, true);
          }
        };
      });
    },

    async departments(root) {
      const canWrite = can("departments:write");
      setActions(canWrite ? `<button class="primary" id="btn-add">新建部门</button>` : "");
      if (canWrite) document.getElementById("btn-add").onclick = () => editDepartment();
      const rows = await api("/api/departments");
      state.cache.departments = rows;
      const byId = Object.fromEntries(rows.map((d) => [d.id, d]));
      root.innerHTML = `<div class="panel">${
        rows.length
          ? `<table class="data"><thead><tr><th>名称</th><th>上级</th><th>排序</th><th>状态</th><th></th></tr></thead>
            <tbody>${rows
              .map((d) => {
                const parent = d.parent_id
                  ? (byId[d.parent_id] && byId[d.parent_id].name) || "—"
                  : "—";
                return `<tr>
              <td>${esc(d.name)}</td>
              <td>${esc(parent)}</td>
              <td>${d.sort_order}</td>
              <td><span class="badge ${d.enabled ? "on" : "off"}">${d.enabled ? "启用" : "停用"}</span></td>
              <td class="actions">${
                canWrite
                  ? `<button data-edit="${d.id}">编辑</button>
                <button class="danger" data-del="${d.id}">删除</button>`
                  : ""
              }</td></tr>`;
              })
              .join("")}</tbody></table>`
          : `<div class="empty">暂无部门。</div>`
      }</div>`;
      root.querySelectorAll("[data-edit]").forEach((b) => {
        b.onclick = () => editDepartment(rows.find((x) => x.id === b.dataset.edit));
      });
      root.querySelectorAll("[data-del]").forEach((b) => {
        b.onclick = async () => {
          if (!confirm("确认删除该部门？")) return;
          try {
            await api(`/api/departments/${b.dataset.del}`, { method: "DELETE" });
            toast("已删除");
            renderPage();
          } catch (e) {
            toast(e.message, true);
          }
        };
      });
    },
  };

  function getAlertView() {
    const v = localStorage.getItem(ALERT_VIEW_KEY);
    return v === "table" ? "table" : "cards";
  }

  function setAlertView(v) {
    localStorage.setItem(ALERT_VIEW_KEY, v === "table" ? "table" : "cards");
  }

  function alertTargetIp(a) {
    const l = a.labels || {};
    return l.alertIp || l.ipaddr || l.instance || l.host || l.hostname || "";
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
          ["alertname", "severity", "source", "alertIp", "ipaddr", "instance", "host", "hostname"],
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
                <span class="sev sev-${esc(a.severity)}">${esc(a.severity)}</span>
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
      <td><span class="sev sev-${esc(a.severity)}">${esc(a.severity)}</span></td>
      <td>
        <div class="alert-name">${esc(name)}</div>
        <div class="alert-labels">${labelChips(
          a.labels,
          ["alertname", "severity", "source", "alertIp", "ipaddr", "instance", "host", "hostname"],
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
    return (
      an.description ||
      an.summary ||
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
          const p = await api(`/trap-api/api/policies/${encodeURIComponent(pid)}`);
          objectOids = (p && p.object_oids) || null;
        }
        if (!objectOids || !Object.keys(objectOids).length) {
          const data = await api("/trap-api/api/policies");
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
          <span class="sev sev-${esc(a.severity)}" style="margin-left:8px">${esc(a.severity)}</span>
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
        <div class="hint">试推送使用控制台登录态，不经过接入 Token 校验。</div>
      </div>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button type="button" class="primary" id="m-run">推送</button>
      </div>`, { wide: true });
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
  const DATASOURCE_TYPES = [
    {
      id: "prometheus",
      name: "Prometheus",
      desc: "PromQL 指标查询",
      icon: "PM",
      tone: "orange",
    },
    {
      id: "victoriametrics",
      name: "VictoriaMetrics",
      desc: "兼容 PromQL",
      icon: "VM",
      tone: "blue",
    },
    {
      id: "kafka",
      name: "Kafka",
      desc: "消息管道 · JSON 字段告警",
      icon: "K",
      tone: "amber",
    },
    {
      id: "log",
      name: "Loki 日志",
      desc: "LogQL 日志统计",
      icon: "Lo",
      tone: "teal",
    },
  ];

  function pickDatasourceKind() {
    openModal(`
      <div class="modal-head">
        <h3>选择数据源类型</h3>
        <p class="desc">先选类型，再填写连接信息；规则评估会按类型自动拉数。</p>
      </div>
      <div class="modal-body">
        <div class="type-pick-grid">
          ${DATASOURCE_TYPES.map(
            (t) => `<button type="button" class="type-pick-card" data-kind="${esc(t.id)}">
              <span class="type-pick-ico tone-${esc(t.tone)}" aria-hidden="true">${esc(t.icon)}</span>
              <span class="type-pick-name">${esc(t.name)}</span>
              <span class="type-pick-desc">${esc(t.desc)}</span>
            </button>`
          ).join("")}
        </div>
      </div>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.querySelectorAll(".type-pick-card").forEach((btn) => {
      btn.onclick = () => editDatasource(null, { kind: btn.dataset.kind });
    });
  }

  function editDatasource(row, opts = {}) {
    if (!row && !opts.kind) {
      pickDatasourceKind();
      return;
    }
    const kind = row?.kind || opts.kind || "prometheus";
    const typeMeta = DATASOURCE_TYPES.find((t) => t.id === kind) || DATASOURCE_TYPES[0];
    const opt = row?.options || {};
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑数据源" : "新建数据源"}</h3>
        <p class="desc">填写连接信息，规则评估会按类型自动拉数。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field">
          <label>名称</label>
          <input name="name" required placeholder="例如：生产 Prometheus" value="${esc(row?.name || "")}" />
        </div>
        <div class="field">
          <label>类型</label>
          <input type="hidden" name="kind" value="${esc(kind)}" />
          <div class="type-picked">
            <span class="type-pick-ico tone-${esc(typeMeta.tone)}" aria-hidden="true">${esc(
              typeMeta.icon
            )}</span>
            <div class="type-picked-text">
              <div class="t-name">${esc(typeMeta.name)}</div>
              <div class="t-desc">${esc(typeMeta.desc)}</div>
            </div>
            ${
              row
                ? ""
                : `<button type="button" class="ghost" id="ds-repick">重选类型</button>`
            }
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
              <input name="topic" placeholder="orders" value="${esc(opt.topic || "")}" />
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
      </div>`, { wide: true });

    const form = document.getElementById("f");
    const syncKind = () => {
      form.querySelectorAll(".kind-panel").forEach((p) => {
        const kinds = (p.dataset.kinds || "").split(",");
        p.hidden = !kinds.includes(kind);
      });
      const urlLabel = document.getElementById("url-label");
      const urlHint = document.getElementById("url-hint");
      const httpInput = form.querySelector('[name="url_http"]');
      if (!urlLabel || !httpInput) return;
      if (kind === "log") {
        urlLabel.textContent = "Loki 地址";
        httpInput.placeholder = "http://127.0.0.1:3100";
        urlHint.textContent = "填写 Loki 根地址。规则表达式使用 LogQL。";
      } else if (kind === "victoriametrics") {
        urlLabel.textContent = "VictoriaMetrics 地址";
        httpInput.placeholder = "http://127.0.0.1:8428";
        urlHint.textContent = "兼容 PromQL 的查询入口。";
      } else {
        urlLabel.textContent = "Prometheus 地址";
        httpInput.placeholder = "http://127.0.0.1:9090";
        urlHint.textContent = "填写 Prometheus 查询 API 根地址。";
      }
    };
    syncKind();

    document.getElementById("m-cancel").onclick = closeModal;
    const repick = document.getElementById("ds-repick");
    if (repick) repick.onclick = () => pickDatasourceKind();
    form.onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(form);
      const k = String(fd.get("kind") || kind || "prometheus");
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

  function showNotifyLogDetail(n) {
    if (!n) return;
    const edge =
      n.transition === "became_firing"
        ? "触发通知"
        : n.transition === "became_resolved"
        ? "恢复通知"
        : n.error === "test"
        ? "渠道测试"
        : n.transition || "其他";
    openModal(`
      <div class="modal-head">
        <h3>通知内容</h3>
        <p class="desc">
          <span class="badge ${n.success ? "on" : "firing"}">${n.success ? "成功" : "失败"}</span>
          <span style="margin-left:8px">${esc(edge)}</span>
          <span style="margin-left:8px">${esc(n.channel_name || "")}</span>
          <span class="hint" style="margin-left:8px">${esc(fmtTime(n.created_at))}</span>
        </p>
      </div>
      <div class="modal-body">
        ${
          n.error
            ? `<div class="field"><label>状态 / 错误</label><div class="summary-box mono">${esc(
                n.error
              )}</div></div>`
            : ""
        }
        <div class="field">
          <label>告警 ID</label>
          <div class="summary-box mono">${esc(
            n.alert_id && n.alert_id !== "00000000-0000-0000-0000-000000000000"
              ? n.alert_id
              : "—"
          )}</div>
        </div>
        <div class="field">
          <label>发送正文</label>
          <pre class="summary-box mono">${esc(n.body || "(无正文)")}</pre>
        </div>
      </div>
      <div class="modal-actions">
        <button type="button" class="primary" id="m-close">关闭</button>
      </div>`);
    document.getElementById("m-close").onclick = closeModal;
  }

  function prettyJson(v) {
    try {
      if (typeof v === "string") {
        const t = v.trim();
        if (!t) return "(空)";
        try {
          return JSON.stringify(JSON.parse(t), null, 2);
        } catch {
          return v;
        }
      }
      return JSON.stringify(v ?? null, null, 2);
    } catch {
      return String(v ?? "");
    }
  }

  function showChannelTestResult(res) {
    const ok = !!res.ok;
    openModal(`
      <div class="modal-head">
        <h3>渠道测试${ok ? "成功" : "失败"}</h3>
        <p class="desc">${esc(res.hint || "")}</p>
      </div>
      <div class="modal-body">
        ${
          res.error
            ? `<div class="field"><label>错误</label><div class="summary-box" style="color:var(--crit)">${esc(
                res.error
              )}</div></div>`
            : ""
        }
        <div class="field">
          <label>发送正文</label>
          <pre class="summary-box mono">${esc(res.text || "")}</pre>
        </div>
        <div class="field">
          <label>请求 URL</label>
          <pre class="summary-box mono">${esc(res.request_url || "")}</pre>
        </div>
        <div class="field">
          <label>请求报文</label>
          <pre class="summary-box mono">${esc(prettyJson(res.request_body))}</pre>
        </div>
        <div class="field">
          <label>HTTP 状态</label>
          <div class="summary-box mono">${esc(
            res.http_status != null ? String(res.http_status) : "(无响应)"
          )}</div>
        </div>
        <div class="field">
          <label>返回内容</label>
          <pre class="summary-box mono">${esc(prettyJson(res.response_body))}</pre>
        </div>
      </div>
      <div class="modal-actions">
        <button type="button" class="primary" id="m-close">关闭</button>
      </div>`);
    document.getElementById("m-close").onclick = closeModal;
  }

  async function testChannel(id, btn) {
    if (!id) return;
    if (btn) btn.disabled = true;
    try {
      const res = await api(`/api/channels/${id}/test`, { method: "POST" });
      toast(res.ok ? res.hint || "测试成功" : res.error || "测试失败", !res.ok);
      showChannelTestResult(res);
    } catch (e) {
      toast(e.message || "测试失败", true);
    } finally {
      if (btn) btn.disabled = false;
    }
  }

  function editChannel(row) {
    const opt = row?.options || {};
    const tplFire = opt.template_firing || "";
    const tplResolve = opt.template_resolved || "";
    const jsonFire = opt.json_firing || "";
    const jsonResolve = opt.json_resolved || "";
    const kindOpts = [
      { id: "webhook", label: "Webhook（固定 Eventide JSON）" },
      { id: "http", label: "自定义 HTTP JSON" },
      { id: "dingtalk", label: "钉钉" },
      { id: "wecom", label: "企业微信" },
      { id: "feishu", label: "飞书" },
      { id: "slack", label: "Slack" },
      { id: "telegram", label: "Telegram" },
    ];
    const chips = [
      "title",
      "transition",
      "severity",
      "status",
      "value",
      "rule.name",
      "fingerprint",
      "labels",
      "annotations",
      "annotations.summary",
      "annotations.summary|json",
      "labels.instance",
      "labels.alertname",
    ];
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑渠道" : "新建通知渠道"}</h3>
        <p class="desc">告警边沿触发时，向所选渠道发送通知。可自定义正文模板与消息格式。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>名称</label><input name="name" required placeholder="例如：值班钉钉群" value="${esc(
          row?.name || ""
        )}" /></div>
        <div class="field"><label>类型</label>
          <select name="kind" id="ch-kind">
            ${kindOpts
              .map(
                (k) =>
                  `<option value="${k.id}" ${
                    row?.kind === k.id || (!row && k.id === "webhook") ? "selected" : ""
                  }>${k.label}</option>`
              )
              .join("")}
          </select>
        </div>
        <div class="field" id="ch-url-field">
          <label id="ch-url-label">Webhook / 机器人 URL</label>
          <input name="url" required value="${esc(row?.url || "")}" placeholder="https://..." />
          <div class="hint" id="ch-url-hint"></div>
        </div>
        <div class="field" id="ch-method-field" hidden>
          <label>HTTP 方法</label>
          <select name="http_method">
            ${["POST", "PUT", "PATCH"]
              .map(
                (m) =>
                  `<option value="${m}" ${
                    (opt.http_method || "POST").toUpperCase() === m ? "selected" : ""
                  }>${m}</option>`
              )
              .join("")}
          </select>
        </div>
        <div class="field" id="ch-secret-field">
          <label id="ch-secret-label">签名密钥（可选）</label>
          <input name="secret" value="${esc(row?.secret || "")}" placeholder="钉钉 / 飞书 secret" />
          <div class="hint" id="ch-secret-hint"></div>
        </div>
        <div class="field" id="ch-headers-field" hidden>
          <label>自定义请求头（JSON 对象）</label>
          <textarea name="headers_json" rows="3" placeholder='{"X-Token":"xxx","Content-Type":"application/json"}'>${esc(
            opt.headers_json || ""
          )}</textarea>
        </div>
        <div class="field" id="ch-chat-field" hidden>
          <label>Telegram chat_id</label>
          <input name="chat_id" value="${esc(opt.chat_id || "")}" placeholder="群/用户 ID，如 -100123... 或 123456" />
        </div>
        <div class="field" id="ch-msg-field">
          <label>消息格式</label>
          <select name="msg_type" id="ch-msg-type">
            <option value="text" ${!opt.msg_type || opt.msg_type === "text" ? "selected" : ""}>纯文本 text</option>
            <option value="markdown" ${opt.msg_type === "markdown" ? "selected" : ""}>Markdown（钉钉/企微/飞书卡片/Slack）</option>
          </select>
          <div class="hint">企微 Markdown 可用 <code>&lt;font&gt;</code>；飞书为卡片 lark_md；Telegram Markdown 按 HTML 解析。</div>
        </div>
        <div class="field" id="ch-at-field">
          <label class="check-row">
            <input type="checkbox" name="at_all" ${
              opt.at_all === "1" || opt.at_all === "true" ? "checked" : ""
            } />
            <span>@所有人（钉钉 / 企微 text）</span>
          </label>
          <input name="at_mobiles" style="margin-top:8px" value="${esc(
            opt.at_mobiles || ""
          )}" placeholder="艾特手机号，逗号分隔（钉钉 / 企微 text）" />
        </div>
        <div class="field">
          <label class="check-row">
            <input type="checkbox" name="enabled" ${!row || row.enabled ? "checked" : ""} />
            <span>启用此渠道</span>
          </label>
        </div>
        <div class="seg" id="seg-text-tpl">
          <div class="seg-title">通知模板</div>
          <div class="hint" style="margin-bottom:10px">留空则用默认正文。语法与丰富规则相同。点芯片可插入到当前模板框。</div>
          <div class="chip-row tpl-chips" style="margin-bottom:12px">
            ${chips
              .map(
                (c) =>
                  `<button type="button" class="chip-tag" data-chip="${esc(c)}">{{${esc(
                    c
                  )}}}</button>`
              )
              .join("")}
          </div>
          <div class="field">
            <label>触发通知模板（firing）</label>
            <textarea name="template_firing" rows="6" placeholder="[{{severity}}] {{rule.name}}&#10;状态: {{transition}}&#10;{{annotations.summary}}&#10;值: {{value}}">${esc(
              tplFire
            )}</textarea>
          </div>
          <div class="field">
            <label>恢复通知模板（resolved）</label>
            <textarea name="template_resolved" rows="5" placeholder="[恢复] {{rule.name}}&#10;{{annotations.summary}}">${esc(
              tplResolve
            )}</textarea>
          </div>
        </div>
        <div class="seg" id="seg-json-tpl" hidden>
          <div class="seg-title">自定义 JSON 报文</div>
          <div class="hint" style="margin-bottom:10px">
            必须是合法 JSON。字符串字段请用 <code>{{annotations.summary|json}}</code>（自带引号与转义）；
            对象可用 <code>{{labels}}</code> / <code>{{annotations}}</code>。留空则发送默认 Eventide 结构。
          </div>
          <div class="chip-row tpl-chips" style="margin-bottom:12px">
            ${chips
              .map(
                (c) =>
                  `<button type="button" class="chip-tag" data-chip="${esc(c)}">{{${esc(
                    c
                  )}}}</button>`
              )
              .join("")}
          </div>
          <div class="field">
            <label>触发 JSON（firing）</label>
            <textarea name="json_firing" rows="8" class="mono" placeholder='{"msg": {{annotations.summary|json}}, "severity": {{severity|json}}, "labels": {{labels}}}'>${esc(
              jsonFire
            )}</textarea>
          </div>
          <div class="field">
            <label>恢复 JSON（resolved）</label>
            <textarea name="json_resolved" rows="6" class="mono" placeholder='{"msg": {{annotations.summary|json}}, "status": "resolved"}'>${esc(
              jsonResolve
            )}</textarea>
          </div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        ${
          row
            ? `<button type="button" class="ghost" id="m-test">测试发送</button>`
            : ""
        }
        <button class="primary" type="submit" form="f">保存</button>
      </div>`, { wide: true });
    document.getElementById("m-cancel").onclick = closeModal;
    const testBtn = document.getElementById("m-test");
    if (testBtn) testBtn.onclick = () => testChannel(row.id, testBtn);

    const syncKindUi = () => {
      const k = document.getElementById("ch-kind")?.value || "webhook";
      const urlLabel = document.getElementById("ch-url-label");
      const urlHint = document.getElementById("ch-url-hint");
      const urlInput = document.querySelector('[name="url"]');
      const secretField = document.getElementById("ch-secret-field");
      const secretLabel = document.getElementById("ch-secret-label");
      const secretHint = document.getElementById("ch-secret-hint");
      const chatField = document.getElementById("ch-chat-field");
      const msgField = document.getElementById("ch-msg-field");
      const atField = document.getElementById("ch-at-field");
      const methodField = document.getElementById("ch-method-field");
      const headersField = document.getElementById("ch-headers-field");
      const textSeg = document.getElementById("seg-text-tpl");
      const jsonSeg = document.getElementById("seg-json-tpl");

      const isHttp = k === "http";
      secretField.hidden = !(k === "dingtalk" || k === "feishu" || isHttp);
      chatField.hidden = k !== "telegram";
      msgField.hidden = k === "webhook" || isHttp;
      atField.hidden = !(k === "dingtalk" || k === "wecom");
      methodField.hidden = !isHttp;
      headersField.hidden = !isHttp;
      textSeg.hidden = isHttp;
      jsonSeg.hidden = !isHttp;

      if (k === "telegram") {
        urlLabel.textContent = "Bot API 基址";
        urlInput.placeholder = "https://api.telegram.org/bot<token>";
        urlHint.textContent = "填 Bot Token 基址即可，会自动追加 /sendMessage；chat_id 必填。";
      } else if (k === "slack") {
        urlLabel.textContent = "Slack Incoming Webhook";
        urlInput.placeholder = "https://hooks.slack.com/services/...";
        urlHint.textContent = "Slack 应用 Incoming Webhooks 生成的 URL。";
      } else if (isHttp) {
        urlLabel.textContent = "HTTP 接口地址";
        urlInput.placeholder = "https://example.com/hooks/alert";
        urlHint.textContent = "向该地址发送 JSON；可用自定义请求头与 Bearer Token。";
        secretLabel.textContent = "Bearer Token（可选）";
        document.querySelector('[name="secret"]').placeholder = "Authorization: Bearer …";
        secretHint.textContent = "若填写，将自动加 Authorization: Bearer &lt;secret&gt;。";
      } else {
        urlLabel.textContent = "Webhook / 机器人 URL";
        urlInput.placeholder = "https://...";
        urlHint.textContent = "";
        secretLabel.textContent = "签名密钥（可选）";
        document.querySelector('[name="secret"]').placeholder = "钉钉 / 飞书 secret";
        secretHint.textContent = "";
      }
    };
    document.getElementById("ch-kind").onchange = syncKindUi;
    syncKindUi();

    let lastTpl = document.querySelector('[name="template_firing"],[name="json_firing"]');
    document
      .querySelectorAll(
        '[name="template_firing"],[name="template_resolved"],[name="json_firing"],[name="json_resolved"]'
      )
      .forEach((el) => {
        el.addEventListener("focus", () => {
          lastTpl = el;
        });
      });
    document.querySelectorAll(".tpl-chips [data-chip]").forEach((b) => {
      b.onclick = () => {
        const el =
          lastTpl ||
          document.querySelector(
            document.getElementById("seg-json-tpl")?.hidden
              ? '[name="template_firing"]'
              : '[name="json_firing"]'
          );
        if (!el) return;
        const token = `{{${b.dataset.chip}}}`;
        const start = el.selectionStart ?? el.value.length;
        const end = el.selectionEnd ?? start;
        el.value = el.value.slice(0, start) + token + el.value.slice(end);
        el.focus();
        const pos = start + token.length;
        el.setSelectionRange(pos, pos);
      };
    });

    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const form = e.target;
      const fd = new FormData(form);
      const secret = String(fd.get("secret") || "").trim();
      const kind = String(fd.get("kind") || "webhook");
      const options = { ...(row?.options || {}) };
      const fire = String(fd.get("template_firing") || "");
      const resolved = String(fd.get("template_resolved") || "");
      if (kind !== "http") {
        if (fire.trim()) options.template_firing = fire;
        else delete options.template_firing;
        if (resolved.trim()) options.template_resolved = resolved;
        else delete options.template_resolved;
        delete options.json_firing;
        delete options.json_resolved;
        delete options.http_method;
        delete options.headers_json;
      } else {
        delete options.template_firing;
        delete options.template_resolved;
        const jf = String(fd.get("json_firing") || "");
        const jr = String(fd.get("json_resolved") || "");
        if (jf.trim()) {
          try {
            JSON.parse(jf.replace(/\{\{[^}]+\}\}/g, "null"));
          } catch {
            // allow templates that aren't valid until rendered; only warn soft
          }
          options.json_firing = jf;
        } else delete options.json_firing;
        if (jr.trim()) options.json_resolved = jr;
        else delete options.json_resolved;
        const method = String(fd.get("http_method") || "POST").toUpperCase();
        if (method && method !== "POST") options.http_method = method;
        else delete options.http_method;
        const headers = String(fd.get("headers_json") || "").trim();
        if (headers) {
          try {
            const obj = JSON.parse(headers);
            if (!obj || typeof obj !== "object" || Array.isArray(obj)) {
              toast("请求头必须是 JSON 对象", true);
              return;
            }
            options.headers_json = JSON.stringify(obj);
          } catch {
            toast("请求头 JSON 无效", true);
            return;
          }
        } else delete options.headers_json;
      }

      const msgType = String(fd.get("msg_type") || "text");
      if (kind !== "webhook" && kind !== "http" && msgType && msgType !== "text")
        options.msg_type = msgType;
      else delete options.msg_type;

      if (kind === "dingtalk" || kind === "wecom") {
        if (form.querySelector('[name="at_all"]')?.checked) options.at_all = "1";
        else delete options.at_all;
        const mobiles = String(fd.get("at_mobiles") || "").trim();
        if (mobiles) options.at_mobiles = mobiles;
        else delete options.at_mobiles;
      } else {
        delete options.at_all;
        delete options.at_mobiles;
      }

      if (kind === "telegram") {
        const chatId = String(fd.get("chat_id") || "").trim();
        if (!chatId) {
          toast("Telegram 请填写 chat_id", true);
          return;
        }
        options.chat_id = chatId;
      } else {
        delete options.chat_id;
      }

      const body = {
        name: fd.get("name"),
        kind,
        url: fd.get("url"),
        secret: secret || null,
        options,
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
                ${severityOptions(row?.severity || "warning")}
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
          <div class="field"><label>注解 annotations（可选，JSON，支持模板）</label>
            <textarea name="annotations" rows="3" placeholder='{"summary":"{{labels.instance}} 超阈值 {{value}}"}'>${esc(
              Object.keys(row?.annotations || {}).length
                ? JSON.stringify(row.annotations, null, 2)
                : ""
            )}</textarea>
            <div class="hint">通知前自动渲染 {{labels.x}} / {{value}} / {{severity}} 等变量。</div>
          </div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`, { wide: true });
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const form = e.target;
      const fd = new FormData(form);
      let labels = {};
      let annotations = {};
      const rawLabels = String(fd.get("labels") || "").trim();
      if (rawLabels) {
        try {
          labels = JSON.parse(rawLabels);
        } catch {
          toast("标签 JSON 无效", true);
          return;
        }
      }
      const rawAnn = String(fd.get("annotations") || "").trim();
      if (rawAnn) {
        try {
          annotations = JSON.parse(rawAnn);
        } catch {
          toast("注解 JSON 无效", true);
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
        annotations,
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

  const INGRESS_TYPES = [
    {
      id: "alertmanager",
      name: "Alertmanager",
      desc: "Prometheus 告警 Webhook",
      icon: "AM",
      tone: "orange",
    },
    {
      id: "generic",
      name: "Generic / 拨测",
      desc: "通用 JSON · 自定义字段映射",
      icon: "{}",
      tone: "blue",
    },
    {
      id: "kafka",
      name: "Kafka",
      desc: "告警总线 Topic 消费",
      icon: "K",
      tone: "amber",
    },
  ];

  function pickIngressKind() {
    openModal(`
      <div class="modal-head">
        <h3>选择接入类型</h3>
        <p class="desc">先选来源类型，再填写连接与字段映射。</p>
      </div>
      <div class="modal-body">
        <div class="type-pick-grid">
          ${INGRESS_TYPES.map(
            (t) => `<button type="button" class="type-pick-card" data-kind="${esc(t.id)}">
              <span class="type-pick-ico tone-${esc(t.tone)}" aria-hidden="true">${esc(t.icon)}</span>
              <span class="type-pick-name">${esc(t.name)}</span>
              <span class="type-pick-desc">${esc(t.desc)}</span>
            </button>`
          ).join("")}
        </div>
      </div>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.querySelectorAll(".type-pick-card").forEach((btn) => {
      btn.onclick = () => editIngress(null, { kind: btn.dataset.kind });
    });
  }

  async function editIngress(row, opts = {}) {
    if (!row && !opts.kind) {
      pickIngressKind();
      return;
    }
    const chs = state.cache.channels || (await api("/api/channels"));
    const kind = row?.kind || opts.kind || "alertmanager";
    const typeMeta = INGRESS_TYPES.find((t) => t.id === kind) || INGRESS_TYPES[0];
    const opt = row?.options || {};
    const mapOn = !!(
      opt.map_status ||
      opt.map_name ||
      opt.map_description ||
      opt.map_ip ||
      opt.map_value ||
      opt.map_fingerprint ||
      opt.map_list ||
      opt.map_enabled === "1"
    );
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑告警接入" : "新建告警接入"}</h3>
        <p class="desc">接入外部已判定的告警。非标准格式可配置字段映射。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field">
          <label>名称</label>
          <input name="name" required placeholder="例如：生产 AM" value="${esc(row?.name || "")}" />
        </div>
        <div class="field">
          <label>类型</label>
          <input type="hidden" name="kind" value="${esc(kind)}" />
          <div class="type-picked">
            <span class="type-pick-ico tone-${esc(typeMeta.tone)}" aria-hidden="true">${esc(
              typeMeta.icon
            )}</span>
            <div class="type-picked-text">
              <div class="t-name">${esc(typeMeta.name)}</div>
              <div class="t-desc">${esc(typeMeta.desc)}</div>
            </div>
            ${
              row
                ? ""
                : `<button type="button" class="ghost" id="ing-repick">重选类型</button>`
            }
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

        <div class="kind-panel" data-kinds="generic,kafka" id="ing-mapping" hidden>
          <div class="seg">
            <div class="seg-title">字段映射（可选）</div>
            <div class="hint" style="margin-bottom:12px">
              填写对方 JSON 的点分路径（如 <code>data.title</code>）。可用变换截取字符串，例如
              <code>sourceciname|before:_</code> → <code>82.12.161.32</code>。
              任一路径非空即启用映射，并优先于内置 Generic/拨测解析。
            </div>
            <details class="map-help">
              <summary>字段说明（点开查看）</summary>
              <div class="map-help-body">
                <p>路径填原始 JSON 字段；可用 <code>|before:</code> / <code>|after:</code> / <code>|split:SEP:INDEX</code> / <code>|between:起点:终点</code> 截取。</p>
                <table class="map-help-table">
                  <thead>
                    <tr><th>配置项</th><th>作用</th><th>写入结果</th></tr>
                  </thead>
                  <tbody>
                    <tr><td><code>map_list</code></td><td>告警数组路径，空=整条消息当一条</td><td>—</td></tr>
                    <tr><td><code>map_status</code></td><td>状态字段</td><td>firing / resolved</td></tr>
                    <tr><td><code>map_fire</code></td><td>视为触发的取值（逗号分隔）</td><td>—</td></tr>
                    <tr><td><code>map_resolve</code></td><td>视为恢复的取值</td><td>—</td></tr>
                    <tr><td><code>map_name</code></td><td>告警名称</td><td><code>labels.alertname</code></td></tr>
                    <tr><td><code>map_description</code></td><td>告警描述</td><td><code>annotations.summary</code> / <code>description</code></td></tr>
                    <tr><td><code>map_ip</code></td><td>告警 IP</td><td><code>labels.ip</code> / <code>alertIp</code> / <code>instance</code></td></tr>
                    <tr><td><code>map_value</code></td><td>当前值</td><td><code>value</code></td></tr>
                    <tr><td><code>map_fingerprint</code></td><td>去重标识</td><td><code>fingerprint</code></td></tr>
                    <tr><td><code>map_severity</code></td><td>级别原始值</td><td><code>labels.severity</code> + 引擎级别</td></tr>
                    <tr><td><code>map_critical</code></td><td>哪些取值算 Disaster/High</td><td>→ disaster</td></tr>
                    <tr><td><code>map_labels</code></td><td>额外标签，<code>目标标签:源路径,...</code></td><td>对应 <code>labels.*</code></td></tr>
                    <tr><td><code>map_enabled</code></td><td>强制开启映射</td><td>—</td></tr>
                  </tbody>
                </table>
                <p class="hint" style="margin:10px 0 0">引擎另支持 <code>map_warning</code>（警告取值列表），可在高级 options 中配置；控制台暂无单独输入框。</p>
              </div>
            </details>
            <div class="field">
              <label class="check-row">
                <input type="checkbox" name="map_enabled" ${mapOn ? "checked" : ""} />
                <span>启用自定义字段映射</span>
              </label>
            </div>
            <div class="row">
              <div class="field">
                <label>告警列表路径 map_list</label>
                <input name="map_list" placeholder="空=整条；或 data.items" value="${esc(opt.map_list || "")}" />
              </div>
              <div class="field">
                <label>状态字段 map_status</label>
                <input name="map_status" placeholder="state / status / eventType" value="${esc(
                  opt.map_status || ""
                )}" />
              </div>
            </div>
            <div class="row">
              <div class="field">
                <label>触发取值 map_fire</label>
                <input name="map_fire" placeholder="默认 fire,firing,ALARM…" value="${esc(opt.map_fire || "")}" />
              </div>
              <div class="field">
                <label>恢复取值 map_resolve</label>
                <input name="map_resolve" placeholder="默认 recover,resolved,OK…" value="${esc(
                  opt.map_resolve || ""
                )}" />
              </div>
            </div>
            <div class="row">
              <div class="field">
                <label>告警名称 map_name</label>
                <input name="map_name" placeholder="title / alertName" value="${esc(opt.map_name || "")}" />
              </div>
              <div class="field">
                <label>告警描述 map_description</label>
                <input name="map_description" placeholder="msg / content" value="${esc(
                  opt.map_description || ""
                )}" />
              </div>
            </div>
            <div class="row">
              <div class="field">
                <label>告警 IP map_ip</label>
                <input name="map_ip" placeholder="sourceciname|before:_" value="${esc(opt.map_ip || "")}" />
                <div class="hint"><code>before:_</code> / <code>after:_</code> / <code>split:_:0</code> / <code>between:起点:终点</code></div>
              </div>
              <div class="field">
                <label>当前值 map_value</label>
                <input name="map_value" placeholder="metric / value" value="${esc(opt.map_value || "")}" />
              </div>
            </div>
            <div class="row">
              <div class="field">
                <label>告警标识 map_fingerprint</label>
                <input name="map_fingerprint" placeholder="id / alertId" value="${esc(
                  opt.map_fingerprint || ""
                )}" />
              </div>
              <div class="field">
                <label>级别 map_severity</label>
                <input name="map_severity" placeholder="level / severity" value="${esc(
                  opt.map_severity || ""
                )}" />
              </div>
            </div>
            <div class="row">
              <div class="field">
                <label>critical 取值</label>
                <input name="map_critical" placeholder="Disaster,High,5,4" value="${esc(opt.map_critical || "")}" />
              </div>
              <div class="field">
                <label>额外标签 map_labels</label>
                <input name="map_labels" placeholder="region:zone,app:appName" value="${esc(
                  opt.map_labels || ""
                )}" />
              </div>
            </div>
          </div>
        </div>

        <div class="field">
          <label>通知渠道</label>
          <select name="channel_id">
            <option value="">不绑定渠道</option>
            ${chs
              .map((c) => {
                const selected = (row?.channel_ids || [])[0] === c.id ? "selected" : "";
                return `<option value="${esc(c.id)}" ${selected}>${esc(c.name)}（${esc(
                  c.kind
                )}）</option>`;
              })
              .join("")}
          </select>
          <div class="hint">未绑定渠道时告警仍会入库，但不会发送通知。</div>
        </div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`, { wide: true });

    const form = document.getElementById("f");
    const syncKind = () => {
      const k = form.querySelector('input[name="kind"]')?.value || "alertmanager";
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
          "默认支持 Generic / 拨测 JSON；其他格式请展开下方「字段映射」配置路径。";
      } else {
        hint.textContent = "";
      }
    };
    syncKind();
    const repick = document.getElementById("ing-repick");
    if (repick) repick.onclick = () => pickIngressKind();

    document.getElementById("m-cancel").onclick = closeModal;
    form.onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(form);
      const k = String(fd.get("kind") || "alertmanager");
      const options = {};
      let endpoint = "";
      let token = String(fd.get("token") || "").trim();
      const channelId = String(fd.get("channel_id") || "").trim();
      const channelIds = channelId ? [channelId] : [];
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
      if (k === "generic" || k === "kafka") {
        const mapKeys = [
          "map_list",
          "map_status",
          "map_fire",
          "map_resolve",
          "map_name",
          "map_description",
          "map_ip",
          "map_value",
          "map_fingerprint",
          "map_severity",
          "map_critical",
          "map_labels",
        ];
        let anyMap = false;
        for (const key of mapKeys) {
          const v = String(fd.get(key) || "").trim();
          if (v) {
            options[key] = v;
            anyMap = true;
          }
        }
        if (anyMap) {
          options.map_enabled = form.querySelector('[name="map_enabled"]')?.checked
            ? "1"
            : "0";
        }
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
        <h3>告警接入已保存</h3>
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
      </div>`, { wide: true });
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

  async function editLookup(row) {
    const initialText =
      row && Object.keys(row.rows || {}).length
        ? formatLookupText(row.key_label || "ip", row.rows)
        : "";
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑台账数据" : "新建台账数据"}</h3>
        <p class="desc">第一列是匹配键（如 IP），后面各列会补到告警上。表头可用 <code>$列名</code>，键列前加 <code>#</code>；列用 Tab 或空格分隔。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>名称</label>
          <input name="name" required value="${esc(row?.name || "")}" placeholder="例如 主机台账" />
        </div>
        <div class="field"><label>说明（可选）</label>
          <input name="description" value="${esc(row?.description || "")}" placeholder="来源、用途" />
        </div>
        <div class="field"><label>用告警的哪个标签来匹配</label>
          <input name="key_label" id="lookup-key-label" required value="${esc(
            row?.key_label || "ip"
          )}" placeholder="ip" />
          <div class="hint">从告警 labels 取这个键去查表。告警 IP 在 <code>instance</code> 里就填 instance；表头是 <code>$ip</code> 时通常填 ip。</div>
        </div>
        <div class="field"><label>台账内容</label>
          <div style="display:flex;gap:0.5rem;margin-bottom:0.4rem;flex-wrap:wrap">
            <label class="ghost" style="display:inline-flex;align-items:center;gap:0.35rem;cursor:pointer;padding:0.35rem 0.7rem;border:1px solid var(--line);border-radius:6px">
              导入文件
              <input type="file" id="lookup-file" accept=".lookup,.txt,.tsv,text/plain" style="display:none" />
            </label>
            <button type="button" class="ghost" id="lookup-to-json">转为 JSON 预览</button>
          </div>
          <textarea name="text" id="lookup-text" rows="12" required spellcheck="false" placeholder="#$ip&#9;$cabinet&#9;$brand&#9;$usagedesc&#10;21.13.0.32&#9;生产中心机房SC-T06&#9;华为&#9;电子渠道综合前置">${esc(
            initialText
          )}</textarea>
          <div class="hint">也支持 JSON：<code>{"21.1.11.11":{"主机名":"DX-AAM"}}</code></div>
          <pre id="lookup-parse-hint" class="mono" style="margin:0.4rem 0 0;font-size:0.78rem;color:var(--muted);white-space:pre-wrap"></pre>
        </div>
        <label class="check-row"><input type="checkbox" name="enabled" ${
          row?.enabled !== false ? "checked" : ""
        } /> <span>启用</span></label>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存台账</button>
      </div>`, { wide: true });
    document.getElementById("m-cancel").onclick = closeModal;

    const textEl = document.getElementById("lookup-text");
    const keyEl = document.getElementById("lookup-key-label");
    const hintEl = document.getElementById("lookup-parse-hint");

    const refreshHint = () => {
      try {
        const parsed = parseLookupClient(textEl.value);
        hintEl.textContent = `已识别 ${parsed.count} 行 · 键列 ${
          parsed.keyHint || "—"
        } · 共 ${(parsed.columns || []).length || "?"} 列`;
        hintEl.style.color = "var(--muted)";
      } catch (err) {
        hintEl.textContent = String(err.message || err);
        hintEl.style.color = "var(--crit-soft, #c44)";
      }
    };
    textEl.addEventListener("input", refreshHint);
    refreshHint();

    document.getElementById("lookup-file").onchange = async (ev) => {
      const file = ev.target.files && ev.target.files[0];
      if (!file) return;
      const text = await file.text();
      textEl.value = text;
      try {
        const parsed = parseLookupClient(text);
        if (parsed.keyHint) keyEl.value = parsed.keyHint;
        if (!document.querySelector('#f [name="name"]').value) {
          document.querySelector('#f [name="name"]').value = file.name.replace(
            /\.(lookup|txt|tsv)$/i,
            ""
          );
        }
        refreshHint();
        toast(`已导入 ${parsed.count} 行`);
      } catch (err) {
        refreshHint();
        toast(err.message || String(err), true);
      }
    };

    document.getElementById("lookup-to-json").onclick = () => {
      try {
        const parsed = parseLookupClient(textEl.value);
        textEl.value = JSON.stringify(parsed.rows, null, 2);
        refreshHint();
      } catch (err) {
        toast(err.message || String(err), true);
      }
    };

    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const fd = new FormData(e.target);
      const text = String(fd.get("text") || "");
      let parsed;
      try {
        parsed = parseLookupClient(text);
      } catch (err) {
        toast(err.message || String(err), true);
        return;
      }
      if (!parsed.count) {
        toast("未识别到数据行，请检查表头与分隔符", true);
        return;
      }
      const body = {
        name: fd.get("name"),
        description: fd.get("description") || "",
        key_label: fd.get("key_label") || parsed.keyHint || "ip",
        rows: parsed.rows,
        text,
        sync_key_from_text: false,
        enabled: e.target.querySelector('[name="enabled"]').checked,
      };
      try {
        if (row) {
          await api(`/api/lookups/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
        } else {
          await api("/api/lookups", { method: "POST", body: JSON.stringify(body) });
        }
      } catch (err) {
        toast(err.message || String(err), true);
        return;
      }
      closeModal();
      toast(`已保存 ${parsed.count} 行`);
      localStorage.setItem("eventide_enrich_tab", "lookups");
      renderPage();
    };
  }

  /** Client-side parse for Omnibus .lookup or JSON rows. */
  function parseLookupClient(text) {
    const trimmed = String(text || "").trim();
    if (!trimmed) throw new Error("外表内容为空");
    if (trimmed.startsWith("{")) {
      const rows = JSON.parse(trimmed);
      return { keyHint: null, rows, count: Object.keys(rows).length };
    }
    const lines = trimmed
      .split(/\r?\n/)
      .map((l) => l.replace(/\s+$/, ""))
      .filter((l) => {
        const t = l.trim();
        return t && !t.startsWith("//") && !t.startsWith(";");
      });
    if (!lines.length) throw new Error("lookup 文件为空");

    const splitFields = (line) => {
      const s = String(line || "").trim();
      if (!s) return [];
      if (s.includes("\t")) return s.split("\t").map((x) => x.trim());
      return s.split(/\s+/).filter(Boolean);
    };

    const splitRow = (line, ncols) => {
      let fields = splitFields(line);
      if (!ncols) return fields;
      if (fields.length === ncols) return fields;
      if (String(line).includes("\t") || fields.length < ncols) {
        while (fields.length < ncols) fields.push("");
        return fields.slice(0, ncols);
      }
      if (ncols === 1) return [fields.join(" ")];
      if (ncols === 2) return [fields[0], fields.slice(1).join(" ")];
      const last = fields[fields.length - 1];
      const midCols = ncols - 2;
      const midTokens = fields.slice(1, -1);
      const out = [fields[0]];
      if (midTokens.length <= midCols) {
        out.push(...midTokens);
        while (out.length < ncols - 1) out.push("");
      } else {
        const keep = midCols - 1;
        out.push(...midTokens.slice(0, keep));
        out.push(midTokens.slice(keep).join(" "));
      }
      out.push(last);
      return out;
    };

    const header = splitFields(lines[0]);
    let keyIdx = 0;
    let foundKey = false;
    const names = header.map((raw, i) => {
      let s = String(raw).trim();
      const isKey = s.startsWith("#");
      if (isKey) {
        keyIdx = i;
        foundKey = true;
      }
      s = s.replace(/^#/, "").replace(/^\$/, "").trim();
      if (!s) throw new Error(`第 ${i + 1} 列表头无效`);
      return s;
    });
    if (!foundKey) keyIdx = 0;
    const keyHint = names[keyIdx];
    const rows = {};
    for (let li = 1; li < lines.length; li++) {
      const fields = splitRow(lines[li], names.length);
      if (fields.every((f) => !String(f).trim())) continue;
      if (fields.length <= keyIdx) {
        throw new Error(
          `第 ${li + 1} 行字段不足（表头 ${names.length} 列）。请用空格或 Tab 分隔列。`
        );
      }
      const key = String(fields[keyIdx] || "").trim();
      if (!key) continue;
      const attrs = {};
      names.forEach((name, i) => {
        if (i === keyIdx) return;
        const val = String(fields[i] || "").trim();
        if (val) attrs[name] = val;
      });
      rows[key] = attrs;
    }
    return { keyHint, rows, count: Object.keys(rows).length, columns: names };
  }

  function formatLookupText(keyLabel, rows) {
    const attrKeys = new Set();
    Object.values(rows || {}).forEach((attrs) => {
      Object.keys(attrs || {}).forEach((k) => attrKeys.add(k));
    });
    const cols = [...attrKeys].sort();
    const header = [`#$${keyLabel}`, ...cols.map((k) => `$${k}`)].join("\t");
    const body = Object.entries(rows || {})
      .map(([key, attrs]) => [key, ...cols.map((c) => attrs[c] || "")].join("\t"))
      .join("\n");
    return body ? `${header}\n${body}\n` : `${header}\n`;
  }


  async function openEnrichPreviewModal(opts = {}) {
    const draft = opts.draft || null;
    const ruleId = opts.ruleId || null;
    const fromEditor = !!opts.fromEditor;
    const samplePayload = `{
  "severity": 4,
  "summary": "麒麟主机当前系统磁盘[vdb] IO使用百分比为: 97.49 %, 已超过90%阈值",
  "lastoccurrence": "2026-07-18 08:39:33",
  "status": 2,
  "sourceid": 1,
  "sourceeventid": "71978",
  "sourceciname": "82.12.161.32_kylin",
  "sourcealertkey": "vfs.dev.util[vdb]",
  "sourceseverity": "High",
  "sourceidentifier": "82.12.161.32_kylin_vfs.dev.util[vdb]_Application:Disk vdb",
  "ciinstance": "Application:Disk vdb",
  "eventtypeid": "*UNKNOWN*"
}`;
    let savedPayload = "";
    try {
      const stash = JSON.parse(sessionStorage.getItem("eventide_enrich_preview") || "{}");
      savedPayload = stash.payload || "";
    } catch (_) {}

    const [rules, ingressRoutes] = await Promise.all([
      api("/api/enrich"),
      api("/api/ingress").catch(() => []),
    ]);
    const mappedIngress = (ingressRoutes || []).filter((r) => {
      const o = r.options || {};
      return !!(
        o.map_status ||
        o.map_name ||
        o.map_description ||
        o.map_ip ||
        o.map_fingerprint ||
        o.map_enabled === "1"
      );
    });
    const defaultIngress =
      mappedIngress.find((r) => /zabbix/i.test(r.name || "")) || mappedIngress[0] || null;

    openModal(
      `
      <div class="modal-head">
        <h3>试跑预览</h3>
        <p class="desc">粘贴原始告警 JSON，先按接入字段映射解析，再套丰富规则看结果。</p>
      </div>
      <div class="modal-body">
        ${
          draft
            ? `<div class="enrich-preview-banner panel" style="margin-bottom:12px">
                使用<strong>当前编辑草稿</strong>${
                  draft.name ? `「${esc(draft.name)}」` : ""
                }（未保存）
              </div>`
            : ""
        }
        <div class="enrich-preview-layout">
          <div>
            <div class="field">
              <label>字段映射接入</label>
              <select id="pv-ingress">
                <option value="">自动 / 仅 labels 对象</option>
                ${mappedIngress
                  .map(
                    (r) =>
                      `<option value="${esc(r.id)}" ${
                        defaultIngress && r.id === defaultIngress.id ? "selected" : ""
                      }>${esc(r.name)}（${esc(r.kind)}）</option>`
                  )
                  .join("")}
              </select>
            </div>
            <div class="field" ${draft ? "hidden" : ""}>
              <label>丰富规则</label>
              <select id="pv-rule">
                <option value="__all__">全部已启用规则（按优先级）</option>
                ${rules
                  .map(
                    (r) =>
                      `<option value="${esc(r.id)}" ${
                        ruleId && r.id === ruleId ? "selected" : ""
                      }>${esc(r.name)}${r.enabled === false ? "（已停用）" : ""}</option>`
                  )
                  .join("")}
              </select>
            </div>
            <div class="field">
              <label>原始告警 JSON</label>
              <textarea id="pv-payload" rows="14" class="mono">${esc(
                savedPayload || samplePayload
              )}</textarea>
            </div>
          </div>
          <div>
            <div id="preview-summary" class="summary-box">点「预览丰富结果」查看</div>
            <div class="preview-metrics">
              <div class="box"><div class="hint">告警 IP</div><div id="preview-ip" class="mono">—</div></div>
              <div class="box"><div class="hint">级别</div><div id="preview-sev" class="mono">—</div></div>
              <div class="box"><div class="hint">名称</div><div id="preview-name" class="mono">—</div></div>
            </div>
            <pre id="preview-out" class="mono enrich-preview-json"></pre>
          </div>
        </div>
      </div>
      <div class="modal-actions">
        ${
          fromEditor
            ? `<button type="button" class="ghost" id="pv-back-edit">返回编辑</button>`
            : `<button type="button" class="ghost" id="pv-close">关闭</button>`
        }
        <button type="button" class="primary" id="pv-run">预览丰富结果</button>
      </div>
    `,
      { xl: true }
    );

    const persistPayload = () => {
      try {
        sessionStorage.setItem(
          "eventide_enrich_preview",
          JSON.stringify({ payload: document.getElementById("pv-payload").value })
        );
      } catch (_) {}
    };

    document.getElementById("pv-run").onclick = async () => {
      let payload;
      try {
        payload = JSON.parse(document.getElementById("pv-payload").value || "{}");
      } catch (e) {
        toast("JSON 无效：" + e.message, true);
        return;
      }
      persistPayload();
      const ingressId = String(document.getElementById("pv-ingress").value || "").trim();
      const body = {
        payload,
        annotations: {},
        value: 1,
        severity: "information",
        rule_name: "PreviewRule",
      };
      if (ingressId) body.ingress_id = ingressId;

      if (draft) {
        body.rule = draft;
      } else {
        const ruleSel = document.getElementById("pv-rule").value;
        if (ruleSel === "__all__") {
          body.use_saved = true;
        } else {
          const r = rules.find((x) => x.id === ruleSel);
          if (!r) {
            toast("请选择丰富规则", true);
            return;
          }
          body.rule = {
            name: r.name,
            kind: r.kind || "auto",
            matchers: r.matchers || {},
            match_key: r.match_key || "",
            templates: r.templates || {},
            mappings: r.mappings || {},
            lookup_table_ids:
              r.lookup_table_ids || (r.lookup_table_id ? [r.lookup_table_id] : []),
            lookup_match_keys: r.lookup_match_keys || {},
            field_templates: r.field_templates || {},
            label_extracts: r.label_extracts || {},
            write_labels: r.write_labels !== false,
            enabled: r.enabled !== false,
            priority: r.priority ?? 100,
          };
        }
      }

      try {
        const out = await api("/api/enrich/preview", {
          method: "POST",
          body: JSON.stringify(body),
        });
        const labels = out.labels || {};
        const an = out.annotations || {};
        document.getElementById("preview-summary").textContent =
          an.summary || an.description || "（未生成描述）";
        document.getElementById("preview-ip").textContent =
          labels.alertIp || labels.ip || labels.instance || "—";
        document.getElementById("preview-sev").textContent =
          labels.severity || out.severity || "—";
        document.getElementById("preview-name").textContent = labels.alertname || "—";
        document.getElementById("preview-out").textContent = JSON.stringify(
          {
            parsed_via: out.parsed_via,
            before: out.before,
            after: {
              labels: out.labels,
              annotations: out.annotations,
              severity: out.severity,
            },
          },
          null,
          2
        );
      } catch (e) {
        toast(e.message, true);
      }
    };

    const closeBtn = document.getElementById("pv-close");
    if (closeBtn) closeBtn.onclick = closeModal;

    const backBtn = document.getElementById("pv-back-edit");
    if (backBtn) {
      backBtn.onclick = () => {
        persistPayload();
        const base = ruleId ? rules.find((r) => r.id === ruleId) : null;
        editEnrich(base || null, { draft });
      };
    }
  }

  async function editEnrich(row, opts = {}) {
    const draft = opts.draft || null;
    const src = draft
      ? {
          ...(row || {}),
          ...draft,
          id: row?.id,
          lookup_table_ids:
            draft.lookup_table_ids ||
            row?.lookup_table_ids ||
            (row?.lookup_table_id ? [row.lookup_table_id] : []),
          lookup_match_keys: draft.lookup_match_keys || row?.lookup_match_keys || {},
          field_templates: draft.field_templates || row?.field_templates || {},
          label_extracts: draft.label_extracts || row?.label_extracts || {},
          mappings: draft.mappings || row?.mappings || {},
          matchers: draft.matchers || row?.matchers || {},
        }
      : row;
    const lookups = state.cache.lookups || (await api("/api/lookups"));
    state.cache.lookups = lookups;
    const selectedIds = new Set(
      src?.lookup_table_ids?.length
        ? src.lookup_table_ids
        : src?.lookup_table_id
        ? [src.lookup_table_id]
        : []
    );
    const ft = src?.field_templates || {};
    const tpl = { ...(src?.templates || {}) };
    const summaryTpl =
      ft.summary ||
      tpl.summary ||
      (selectedIds.size
        ? "{{labels.样例-主机台账.主机名}}（{{labels.样例-主机台账.别名}}）"
        : "{{labels.alertname}} · {{labels.instance}}");
    delete tpl.summary;
    const otherTplJson = Object.keys(tpl).length ? JSON.stringify(tpl, null, 2) : "";
    const builtinChipFields = ["ip", "instance", "alertIp", "alertname", "severity"];
    const colsFromLookup = (t) => {
      const set = new Set();
      Object.values(t.rows || {}).forEach((attrs) => {
        Object.keys(attrs || {}).forEach((k) => set.add(k));
      });
      return [...set].sort();
    };
    const matcherPairs = Object.entries(src?.matchers || {}).map(([k, v]) => ({ k, v }));
    if (!matcherPairs.length) matcherPairs.push({ k: "", v: "" });

    const matcherRowsHtml = (pairs) =>
      pairs
        .map(
          (p, i) => `<div class="matcher-row" data-idx="${i}">
            <input name="mk_key" placeholder="标签名，如 job" value="${esc(p.k)}" />
            <span class="matcher-eq">=</span>
            <input name="mk_val" placeholder="值，如 api" value="${esc(p.v)}" />
            <button type="button" class="ghost" data-rm-matcher title="删除">×</button>
          </div>`
        )
        .join("");

    const hasAdvanced =
      (src?.mappings && Object.keys(src.mappings).length) ||
      otherTplJson ||
      (src?.priority != null && src.priority !== 100) ||
      src?.write_labels === false;

    openModal(
      `
      <div class="modal-head">
        <h3>${row ? "编辑丰富规则" : "新建丰富规则"}</h3>
        <p class="desc">选台账补字段，再决定告警上显示的描述、IP 和级别。保存前可点「试跑预览」验证。</p>
      </div>
      <form id="f" class="modal-body">
        <details class="map-help enrich-help">
          <summary>帮助说明：可用字段与语法</summary>
          <div class="map-help-body">
            <p><b>执行顺序</b>：查表前抽取 → 台账查表 → 内联映射 → 注解模板 → 写到告警卡片（描述/IP/级别/名称）。</p>

            <p><b>模板变量</b>（可写在抽取模板、告警描述、IP、级别、名称等处）</p>
            <table class="map-help-table">
              <thead>
                <tr><th>写法</th><th>含义</th></tr>
              </thead>
              <tbody>
                <tr><td><code>{{labels.xxx}}</code></td><td>告警标签；常见有 <code>ip</code> / <code>alertIp</code> / <code>instance</code> / <code>alertname</code> / <code>severity</code>，以及接入 <code>map_labels</code> 写入的字段</td></tr>
                <tr><td><code>{{labels.台账名.列名}}</code></td><td>台账查出的列（必须带台账名前缀，如 <code>{{labels.device_info_form.主机名}}</code>）</td></tr>
                <tr><td><code>{{annotations.xxx}}</code></td><td>告警注解；常见 <code>summary</code> / <code>description</code></td></tr>
                <tr><td><code>{{value}}</code></td><td>当前监控值</td></tr>
                <tr><td><code>{{severity}}</code></td><td>引擎级别（Zabbix：not_classified / information / warning / average / high / disaster）</td></tr>
                <tr><td><code>{{status}}</code></td><td>firing / resolved</td></tr>
                <tr><td><code>{{fingerprint}}</code></td><td>告警指纹</td></tr>
                <tr><td><code>{{rule.name}}</code></td><td>当前丰富规则名</td></tr>
              </tbody>
            </table>

            <p style="margin-top:12px"><b>字符串截取语法</b>（加在变量路径后，用 <code>|</code> 连接）</p>
            <table class="map-help-table">
              <thead>
                <tr><th>语法</th><th>说明</th><th>示例</th></tr>
              </thead>
              <tbody>
                <tr><td><code>|before:SEP</code></td><td>取分隔符<strong>之前</strong></td><td><code>{{labels.sourceciname|before:_}}</code> → <code>82.12.161.32</code></td></tr>
                <tr><td><code>|after:SEP</code></td><td>取分隔符<strong>之后</strong></td><td><code>{{annotations.summary|after:为：}}</code></td></tr>
                <tr><td><code>|split:SEP:INDEX</code></td><td>按分隔符拆分，取第 INDEX 段（从 0 起）</td><td><code>{{labels.x|split:_:0}}</code></td></tr>
                <tr><td><code>|between:起点:终点</code></td><td>取两段之间；起点/终点可空（空=串首/串尾）</td><td><code>{{annotations.summary|between:为：: %}}</code></td></tr>
              </tbody>
            </table>

            <p style="margin-top:12px"><b>各配置项怎么用</b></p>
            <table class="map-help-table">
              <thead>
                <tr><th>配置</th><th>说明</th></tr>
              </thead>
              <tbody>
                <tr><td>何时生效</td><td>按 <code>labels</code> 精确匹配；全部满足才应用本规则；不配=全部告警</td></tr>
                <tr><td>查表前抽取</td><td>用模板算出值，写入 <code>labels.标签名</code>（也可进 annotations），供下方「用标签」查台账。例：标签 <code>sss_ip</code> = <code>{{annotations.summary|before:_}}</code></td></tr>
                <tr><td>从台账补字段</td><td>勾选台账；「用标签」填匹配键（默认台账的 key，可改为抽取的 <code>sss_ip</code>）。命中后写入 <code>labels.台账名.列名</code></td></tr>
                <tr><td>告警描述</td><td>对应 field 模板 <code>summary</code>；可点/拖下方芯片插入</td></tr>
                <tr><td>告警 IP</td><td>写入 <code>ip</code> / <code>alertIp</code>（及必要时 <code>instance</code>）</td></tr>
                <tr><td>告警级别</td><td>Zabbix 六级：disaster / high / average / warning / information / not_classified（兼容 critical→disaster、info→information；也可用中文）</td></tr>
                <tr><td>告警名称</td><td>可选，改写 <code>labels.alertname</code></td></tr>
                <tr><td>高级 · 内联映射</td><td>无台账时用 JSON 对照表；匹配键填标签名</td></tr>
              </tbody>
            </table>

            <p class="hint" style="margin:12px 0 0">推荐路径：接入已映射出 <code>ip</code> → 直接用标签 <code>ip</code> 查台账 → 描述写 <code>{{labels.台账名.主机名}}</code>。若 IP 在描述里，先抽取再把「用标签」改成抽取名。</p>
          </div>
        </details>

        <div class="enrich-step">
          <div class="enrich-step-head">
            <h4>基本信息</h4>
            <p>给规则起个名字；默认对全部告警生效。</p>
          </div>
          <div class="row">
            <div class="field" style="flex:2"><label>名称</label>
              <input name="name" required value="${esc(src?.name || "")}" placeholder="例如 主机台账丰富" />
            </div>
            <div class="field" style="flex:0.8;min-width:120px;align-self:flex-end">
              <label class="check-row" style="margin:0;padding:8px 0"><input type="checkbox" name="enabled" ${
                src?.enabled !== false ? "checked" : ""
              }/><span>启用</span></label>
            </div>
          </div>
          <details class="enrich-when" ${
            Object.keys(src?.matchers || {}).length ? "open" : ""
          }>
            <summary>何时生效（可选）· 默认全部告警</summary>
            <div class="hint" style="margin:10px 0 8px">仅当告警同时带有下列标签时才应用本规则。</div>
            <div id="matcher-rows">${matcherRowsHtml(matcherPairs)}</div>
            <button type="button" class="ghost" id="btn-add-matcher" style="margin-top:8px">添加条件</button>
          </details>
        </div>

        <div class="enrich-step">
          <div class="enrich-step-head">
            <h4>查表前抽取标签（可选）</h4>
            <p>从原告警描述/标签截取后写入 labels，供台账匹配。支持 <code>|before:</code> / <code>|after:</code> / <code>|split:SEP:INDEX</code> / <code>|between:起点:终点</code>。</p>
          </div>
          <div id="extract-rows"></div>
          <button type="button" class="ghost" id="btn-add-extract" style="margin-top:8px">添加抽取</button>
          <div class="hint" style="margin-top:8px">例：标签名 <code>sss_ip</code>，模板 <code>{{annotations.summary|before:_}}</code>；下方台账「用标签」填 <code>sss_ip</code>。</div>
        </div>

        <div class="enrich-step">
          <div class="enrich-step-head">
            <h4>从台账补字段</h4>
            <p>勾选台账，并指定<strong>用哪个标签查表</strong>（可填上方抽取的 <code>sss_ip</code>）。查出的列用 <code>{{labels.台账名.列名}}</code>。</p>
          </div>
          <div class="field">
            <div class="lookup-bind-list" id="lookup-bind-list">
              ${
                lookups.length
                  ? lookups
                      .map((t) => {
                        const checked = selectedIds.has(t.id);
                        const override =
                          (src?.lookup_match_keys && src.lookup_match_keys[t.id]) || "";
                        const defKey = t.key_label || "ip";
                        return `<div class="lookup-bind-row">
                    <label class="check-row lookup-bind-check">
                      <input type="checkbox" name="lookup_table_ids" value="${esc(t.id)}" ${
                          checked ? "checked" : ""
                        } data-default-key="${esc(defKey)}" />
                      <span><b>${esc(t.name)}</b> · ${Object.keys(t.rows || {}).length} 行${
                          t.enabled ? "" : " · 已停用"
                        }</span>
                    </label>
                    <div class="lookup-bind-key">
                      <span class="hint">用标签</span>
                      <input name="lookup_key_${esc(t.id)}" class="mono" placeholder="${esc(
                          defKey
                        )}" value="${esc(override || defKey)}" ${checked ? "" : "disabled"} />
                      <span class="hint">查本表</span>
                    </div>
                  </div>`;
                      })
                      .join("")
                  : `<div class="enrich-inline-empty">
                      还没有台账。
                      <button type="button" class="ghost" id="btn-goto-lookup">去新建台账</button>
                    </div>`
              }
            </div>
          </div>
          <div class="hint" style="margin-top:8px">例：抽取了 <code>sss_ip</code> → 此处填 <code>sss_ip</code> → 描述写 <code>{{labels.device_info_test.主机名}}</code>。</div>
        </div>

        <div class="enrich-step">
          <div class="enrich-step-head">
            <h4>写到告警卡片上</h4>
            <p>用 <code>{{labels.台账名.列名}}</code> 或截取 <code>{{annotations.summary|between:为：: %}}</code>。点芯片插入，或拖到描述任意位置。</p>
          </div>
          <div class="field"><label>告警描述</label>
            <textarea name="ft_summary" id="summary-tpl" rows="3" class="chip-drop-target">${esc(summaryTpl)}</textarea>
            <div class="hint" id="chip-hint" style="margin:8px 0 6px">先勾选台账；芯片可点击或拖入上方描述框。</div>
            <div id="summary-chips" class="chip-groups"></div>
          </div>
          <div class="row">
            <div class="field"><label>告警 IP</label>
              <input name="ft_ip" value="${esc(
                ft.ip || ft.alertIp || "{{labels.ip}}"
              )}" placeholder="{{labels.ip}} 或 {{labels.instance}}" />
            </div>
            <div class="field"><label>告警级别</label>
              <input name="ft_severity" value="${esc(
                ft.severity || ""
              )}" placeholder="disaster / 严重 / {{labels.级别}}" />
            </div>
          </div>
          <div class="field"><label>告警名称（可选）</label>
            <input name="ft_alertname" value="${esc(
              ft.alertname || ""
            )}" placeholder="{{labels.主机名}}-不可达，留空则不改" />
          </div>
          <div class="hint">级别按 Zabbix：disaster / high / average / warning / information / not_classified（兼容 critical、info；可用中文）。</div>
        </div>

        <details class="enrich-step enrich-advanced" ${hasAdvanced ? "open" : ""}>
          <summary>高级选项（内联映射、优先级等）</summary>
          <div class="enrich-advanced-body">
            <div class="row">
              <div class="field" style="flex:1;min-width:160px"><label>内联映射 · 用哪个标签匹配</label>
                <input name="match_key" value="${esc(
                  src?.match_key || ""
                )}" placeholder="留空=各台账用自己的匹配标签" />
                <div class="hint">只影响下方「内联映射表」，与已选台账无关。</div>
              </div>
              <div class="field" style="flex:0.6;min-width:100px"><label>优先级</label>
                <input name="priority" type="number" value="${src?.priority ?? 100}" />
                <div class="hint">数字越小越先执行</div>
              </div>
            </div>
            <div class="field"><label>内联映射表（JSON，可选）</label>
              <textarea name="mappings" rows="4" class="mono" placeholder='{"10.0.0.1":{"owner":"alice"}}'>${esc(
                Object.keys(src?.mappings || {}).length
                  ? JSON.stringify(src.mappings, null, 2)
                  : ""
              )}</textarea>
              <div class="hint">无需台账时，可直接在规则里写一小段对照表。</div>
            </div>
            <div class="field"><label>其他注解模板（JSON，可选）</label>
              <textarea name="templates_extra" rows="3" class="mono">${esc(otherTplJson)}</textarea>
            </div>
            <label class="check-row"><input type="checkbox" name="write_labels" ${
              src?.write_labels !== false ? "checked" : ""
            } /><span>台账 / 映射字段写入 labels（供描述模板引用）</span></label>
          </div>
        </details>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button type="button" class="ghost" id="m-goto-preview">试跑预览</button>
        <button class="primary" type="submit" form="f">保存规则</button>
      </div>
    `,
      { xl: true }
    );

    document.getElementById("m-cancel").onclick = closeModal;

    const gotoLookup = document.getElementById("btn-goto-lookup");
    if (gotoLookup) {
      gotoLookup.onclick = () => {
        closeModal();
        localStorage.setItem("eventide_enrich_tab", "lookups");
        renderPage();
        setTimeout(() => editLookup(), 50);
      };
    }

    const matcherBox = document.getElementById("matcher-rows");
    const bindMatcherRm = () => {
      matcherBox.querySelectorAll("[data-rm-matcher]").forEach((btn) => {
        btn.onclick = () => {
          const rowEl = btn.closest(".matcher-row");
          if (matcherBox.querySelectorAll(".matcher-row").length <= 1) {
            rowEl.querySelector('[name="mk_key"]').value = "";
            rowEl.querySelector('[name="mk_val"]').value = "";
            return;
          }
          rowEl.remove();
        };
      });
    };
    bindMatcherRm();
    document.getElementById("btn-add-matcher").onclick = () => {
      if (matcherBox.querySelectorAll(".matcher-row").length >= 5) {
        toast("最多 5 个条件", true);
        return;
      }
      matcherBox.insertAdjacentHTML(
        "beforeend",
        `<div class="matcher-row">
          <input name="mk_key" placeholder="标签名" value="" />
          <span class="matcher-eq">=</span>
          <input name="mk_val" placeholder="值" value="" />
          <button type="button" class="ghost" data-rm-matcher title="删除">×</button>
        </div>`
      );
      bindMatcherRm();
    };

    const extractBox = document.getElementById("extract-rows");
    const extractPairs = Object.entries(src?.label_extracts || {}).map(([k, v]) => ({
      k,
      v,
    }));
    if (!extractPairs.length) extractPairs.push({ k: "", v: "" });
    const extractRowHtml = (p) => `<div class="extract-row">
      <input name="ex_key" placeholder="写入标签名，如 extract_ip" value="${esc(p.k)}" />
      <input name="ex_tpl" placeholder="{{annotations.summary|before:_}}" value="${esc(p.v)}" />
      <button type="button" class="ghost" data-rm-extract title="删除">×</button>
    </div>`;
    extractBox.innerHTML = extractPairs.map(extractRowHtml).join("");
    const bindExtractRm = () => {
      extractBox.querySelectorAll("[data-rm-extract]").forEach((btn) => {
        btn.onclick = () => {
          const rowEl = btn.closest(".extract-row");
          if (extractBox.querySelectorAll(".extract-row").length <= 1) {
            rowEl.querySelector('[name="ex_key"]').value = "";
            rowEl.querySelector('[name="ex_tpl"]').value = "";
            return;
          }
          rowEl.remove();
        };
      });
    };
    bindExtractRm();
    document.getElementById("btn-add-extract").onclick = () => {
      if (extractBox.querySelectorAll(".extract-row").length >= 8) {
        toast("最多 8 条抽取", true);
        return;
      }
      extractBox.insertAdjacentHTML("beforeend", extractRowHtml({ k: "", v: "" }));
      bindExtractRm();
    };

    const summaryEl = document.getElementById("summary-tpl");
    const chipsEl = document.getElementById("summary-chips");
    const chipHint = document.getElementById("chip-hint");

    const lookupNs = (name) => {
      const s = String(name || "")
        .split("")
        .map((c) => (/[a-zA-Z0-9_\-\u4e00-\u9fff]/.test(c) ? c : "_"))
        .join("");
      return s || "lookup";
    };

    const insertChipInto = (el, chip, at) => {
      if (!el || !chip) return;
      const start =
        at != null ? at : el.selectionStart ?? String(el.value || "").length;
      const end = at != null ? at : el.selectionEnd ?? start;
      const v = String(el.value || "");
      el.value = v.slice(0, start) + chip + v.slice(end);
      el.focus();
      const caret = start + chip.length;
      if (typeof el.setSelectionRange === "function") {
        el.setSelectionRange(caret, caret);
      }
    };

    /** Approximate caret index in a textarea from pointer coordinates. */
    const textareaIndexFromPoint = (ta, clientX, clientY) => {
      const text = String(ta.value || "");
      if (!text.length) return 0;
      const style = getComputedStyle(ta);
      const rect = ta.getBoundingClientRect();
      const mirror = document.createElement("div");
      mirror.setAttribute("aria-hidden", "true");
      mirror.style.cssText = [
        "position:fixed",
        `left:${rect.left}px`,
        `top:${rect.top}px`,
        `width:${ta.clientWidth}px`,
        `height:${ta.clientHeight}px`,
        "overflow:hidden",
        "visibility:hidden",
        "pointer-events:none",
        "white-space:pre-wrap",
        "word-wrap:break-word",
        `font:${style.font}`,
        `font-size:${style.fontSize}`,
        `font-family:${style.fontFamily}`,
        `font-weight:${style.fontWeight}`,
        `line-height:${style.lineHeight}`,
        `letter-spacing:${style.letterSpacing}`,
        `padding:${style.padding}`,
        `border:${style.border}`,
        `box-sizing:${style.boxSizing}`,
      ].join(";");
      document.body.appendChild(mirror);
      mirror.scrollTop = ta.scrollTop;
      mirror.scrollLeft = ta.scrollLeft;

      let best = text.length;
      let bestDist = Infinity;
      const marker = document.createElement("span");
      marker.textContent = "\u200b";
      // Templates are short; O(n) probe is fine.
      for (let i = 0; i <= text.length; i++) {
        mirror.textContent = "";
        mirror.appendChild(document.createTextNode(text.slice(0, i)));
        mirror.appendChild(marker);
        mirror.appendChild(document.createTextNode(text.slice(i)));
        mirror.scrollTop = ta.scrollTop;
        mirror.scrollLeft = ta.scrollLeft;
        const mr = marker.getBoundingClientRect();
        const cx = mr.left;
        const cy = mr.top + mr.height / 2;
        const d = (cx - clientX) ** 2 + (cy - clientY) ** 2;
        if (d < bestDist) {
          bestDist = d;
          best = i;
        }
      }
      mirror.remove();
      return best;
    };

    let chipDragActive = false;
    let suppressChipClick = false;

    const bindChips = () => {
      chipsEl.querySelectorAll("[data-chip]").forEach((btn) => {
        btn.draggable = true;
        btn.ondragstart = (e) => {
          chipDragActive = true;
          suppressChipClick = false;
          const chip = btn.dataset.chip || "";
          e.dataTransfer.setData("text/plain", chip);
          e.dataTransfer.setData("application/x-eventide-chip", chip);
          e.dataTransfer.effectAllowed = "copy";
          btn.classList.add("is-dragging");
        };
        btn.ondragend = () => {
          chipDragActive = false;
          btn.classList.remove("is-dragging");
          // Avoid click-insert after a successful drag.
          suppressChipClick = true;
          setTimeout(() => {
            suppressChipClick = false;
          }, 0);
        };
        btn.onclick = () => {
          if (suppressChipClick) return;
          insertChipInto(summaryEl, btn.dataset.chip);
        };
      });
    };

    summaryEl.addEventListener("dragenter", (e) => {
      const types = e.dataTransfer?.types
        ? Array.from(e.dataTransfer.types)
        : [];
      if (
        !chipDragActive &&
        !types.includes("text/plain") &&
        !types.includes("application/x-eventide-chip")
      ) {
        return;
      }
      e.preventDefault();
      summaryEl.classList.add("is-chip-drag-over");
    });
    summaryEl.addEventListener("dragover", (e) => {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      summaryEl.classList.add("is-chip-drag-over");
      const idx = textareaIndexFromPoint(summaryEl, e.clientX, e.clientY);
      summaryEl.focus();
      summaryEl.setSelectionRange(idx, idx);
    });
    summaryEl.addEventListener("dragleave", (e) => {
      if (e.target !== summaryEl) return;
      summaryEl.classList.remove("is-chip-drag-over");
    });
    summaryEl.addEventListener("drop", (e) => {
      e.preventDefault();
      summaryEl.classList.remove("is-chip-drag-over");
      const chip =
        e.dataTransfer.getData("application/x-eventide-chip") ||
        e.dataTransfer.getData("text/plain") ||
        "";
      if (!chip) return;
      const idx = textareaIndexFromPoint(summaryEl, e.clientX, e.clientY);
      insertChipInto(summaryEl, chip, idx);
    });

    const chipBtn = (labelKey, title, cls = "") =>
      `<button type="button" class="ghost chip-tag ${cls}" draggable="true" data-chip="{{labels.${esc(
        labelKey
      )}}}" title="${esc(title)}（可拖入描述）">{{labels.${esc(labelKey)}}}</button>`;

    const refreshChips = () => {
      const checked = [
        ...document.querySelectorAll('input[name="lookup_table_ids"]:checked'),
      ].map((el) => el.value);

      const parts = [];
      parts.push(`<div class="chip-group">
        <div class="chip-group-title">告警自带标签</div>
        <div class="chip-row">${builtinChipFields
          .map((k) => chipBtn(k, "告警自带，不依赖台账"))
          .join("")}</div>
      </div>`);

      checked.forEach((id) => {
        const t = lookups.find((x) => x.id === id);
        if (!t) return;
        const keyLabel = t.key_label || "ip";
        const ns = lookupNs(t.name);
        const cols = colsFromLookup(t);
        const buttons = cols
          .map((col) =>
            chipBtn(
              `${ns}.${col}`,
              `台账「${t.name}」列「${col}」。匹配：告警 labels.${keyLabel} → 查本表`,
              "chip-from-lookup"
            )
          )
          .join("");
        parts.push(`<div class="chip-group">
          <div class="chip-group-title"><b>${esc(t.name)}</b> · 匹配键 <code class="mono">labels.${esc(
            keyLabel
          )}</code> → 写入 <code class="mono">labels.${esc(ns)}.*</code></div>
          <div class="chip-row">${
            buttons || `<span class="hint">该台账没有数据列</span>`
          }</div>
        </div>`);
      });

      chipsEl.innerHTML = parts.join("");
      if (chipHint) {
        chipHint.textContent = checked.length
          ? "台账字段一律为 {{labels.台账名.列名}}；点选或拖入描述框任意位置。"
          : "先勾选上方台账，才会列出可拖拽的 {{labels.台账名.列名}} 芯片。";
      }
      bindChips();
    };

    document.querySelectorAll('input[name="lookup_table_ids"]').forEach((el) => {
      const syncKey = () => {
        const row = el.closest(".lookup-bind-row");
        const keyInput = row?.querySelector(`input[name="lookup_key_${el.value}"]`);
        if (!keyInput) return;
        keyInput.disabled = !el.checked;
        if (el.checked && !String(keyInput.value || "").trim()) {
          keyInput.value = el.dataset.defaultKey || "ip";
        }
      };
      el.addEventListener("change", () => {
        syncKey();
        refreshChips();
      });
      syncKey();
    });
    refreshChips();

    const collectMatchers = (form) => {
      const matchers = {};
      form.querySelectorAll(".matcher-row").forEach((rowEl) => {
        const k = String(rowEl.querySelector('[name="mk_key"]')?.value || "").trim();
        const v = String(rowEl.querySelector('[name="mk_val"]')?.value || "").trim();
        if (k && v) matchers[k] = v;
      });
      return matchers;
    };

    const collectExtracts = (form) => {
      const out = {};
      form.querySelectorAll(".extract-row").forEach((rowEl) => {
        const k = String(rowEl.querySelector('[name="ex_key"]')?.value || "").trim();
        const v = String(rowEl.querySelector('[name="ex_tpl"]')?.value || "").trim();
        if (k && v) out[k] = v;
      });
      return out;
    };

    const buildDraft = (fd, form) => {
      let mappings = {};
      let extra = {};
      const rawMap = String(fd.get("mappings") || "").trim();
      if (rawMap) mappings = JSON.parse(rawMap);
      const rawExtra = String(fd.get("templates_extra") || "").trim();
      if (rawExtra) extra = JSON.parse(rawExtra);
      const summary = String(fd.get("ft_summary") || "").trim();
      const templates = { ...extra };
      if (summary) templates.summary = summary;
      const field_templates = {};
      if (summary) field_templates.summary = summary;
      const ip = String(fd.get("ft_ip") || "").trim();
      const severity = String(fd.get("ft_severity") || "").trim();
      const alertname = String(fd.get("ft_alertname") || "").trim();
      if (ip) field_templates.ip = ip;
      if (severity) field_templates.severity = severity;
      if (alertname) field_templates.alertname = alertname;
      const lookup_table_ids = [
        ...form.querySelectorAll('input[name="lookup_table_ids"]:checked'),
      ].map((el) => el.value);
      const lookup_match_keys = {};
      lookup_table_ids.forEach((id) => {
        const keyInput = form.querySelector(`input[name="lookup_key_${id}"]`);
        const k = String(keyInput?.value || "").trim();
        if (k) lookup_match_keys[id] = k;
      });
      return {
        name: fd.get("name") || "preview",
        kind: "auto",
        matchers: collectMatchers(form),
        match_key: String(fd.get("match_key") || "").trim(),
        templates,
        mappings,
        lookup_table_ids,
        lookup_match_keys,
        field_templates,
        label_extracts: collectExtracts(form),
        write_labels: form.querySelector('[name="write_labels"]')?.checked || false,
        enabled: form.querySelector('[name="enabled"]')?.checked !== false,
        priority: Number(fd.get("priority") || 100),
      };
    };

    document.getElementById("m-goto-preview").onclick = () => {
      const form = document.getElementById("f");
      const fd = new FormData(form);
      let draftBody;
      try {
        draftBody = buildDraft(fd, form);
        draftBody.name = String(fd.get("name") || "").trim() || draftBody.name;
        draftBody.enabled = form.querySelector('[name="enabled"]').checked;
      } catch (e) {
        toast("高级 JSON 无效：" + e.message, true);
        return;
      }
      openEnrichPreviewModal({
        draft: draftBody,
        ruleId: row?.id || null,
        fromEditor: true,
      });
    };

    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const form = e.target;
      const fd = new FormData(form);
      let body;
      try {
        body = buildDraft(fd, form);
        body.name = fd.get("name");
        body.enabled = form.querySelector('[name="enabled"]').checked;
      } catch (err) {
        toast("高级 JSON 无效：" + err.message, true);
        return;
      }
      const hasAny =
        (body.lookup_table_ids || []).length ||
        Object.keys(body.mappings || {}).length ||
        Object.keys(body.templates || {}).length ||
        Object.keys(body.field_templates || {}).length ||
        Object.keys(body.label_extracts || {}).length;
      if (!hasAny) {
        toast("请至少勾选台账，或配置描述 / IP / 级别", true);
        return;
      }
      if (Object.keys(body.mappings || {}).length && !body.match_key) {
        toast("使用内联映射时，请填写「用哪个标签匹配」", true);
        return;
      }
      try {
        if (row) {
          await api(`/api/enrich/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
        } else {
          await api("/api/enrich", { method: "POST", body: JSON.stringify(body) });
        }
      } catch (err) {
        toast(err.message || String(err), true);
        return;
      }
      sessionStorage.removeItem("eventide_enrich_preview");
      closeModal();
      toast("已保存");
      localStorage.setItem("eventide_enrich_tab", "rules");
      renderPage();
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
      </div>`, { wide: true });
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

  // ---------- IAM editors ----------
  async function editUser(row) {
    const [roles, depts] = await Promise.all([
      state.cache.roles || api("/api/roles"),
      state.cache.departments || api("/api/departments"),
    ]);
    state.cache.roles = roles;
    state.cache.departments = depts;
    const selectedRoles = new Set(row?.role_ids || []);
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑用户" : "新建用户"}</h3>
        <p class="desc">分配部门与角色；角色权限在「权限管理」配置。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="row">
          <div class="field"><label>用户名</label>
            <input name="username" required value="${esc(row?.username || "")}" ${
              row ? "readonly" : ""
            } /></div>
          <div class="field"><label>显示名</label>
            <input name="display_name" value="${esc(row?.display_name || "")}" /></div>
        </div>
        <div class="field"><label>${row ? "新密码（留空不改）" : "初始密码"}</label>
          <input name="password" type="password" autocomplete="new-password" ${
            row ? "" : "required minlength=6"
          } placeholder="${row ? "留空则不修改" : "至少 6 位"}" /></div>
        <div class="field"><label>部门</label>
          <select name="department_id">
            <option value="">— 未分配 —</option>
            ${depts
              .map(
                (d) =>
                  `<option value="${esc(d.id)}" ${
                    row?.department_id === d.id ? "selected" : ""
                  }>${esc(d.name)}</option>`
              )
              .join("")}
          </select>
        </div>
        <div class="field"><label>角色</label>
          <div class="check-grid">${roles
            .map(
              (r) => `<label class="check-row"><input type="checkbox" name="role_ids" value="${esc(
                r.id
              )}" ${selectedRoles.has(r.id) ? "checked" : ""}/> <span>${esc(r.name)}${
                r.is_system ? "（系统）" : ""
              }</span></label>`
            )
            .join("")}</div>
        </div>
        <label class="check-row"><input type="checkbox" name="enabled" ${
          !row || row.enabled ? "checked" : ""
        }/> <span>启用</span></label>
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
      const password = String(fd.get("password") || "").trim();
      const body = {
        username: String(fd.get("username") || "").trim(),
        display_name: String(fd.get("display_name") || "").trim(),
        department_id: String(fd.get("department_id") || "") || null,
        role_ids: [...form.querySelectorAll('[name="role_ids"]:checked')].map((x) => x.value),
        enabled: form.querySelector('[name="enabled"]').checked,
      };
      if (password) body.password = password;
      try {
        if (row) {
          await api(`/api/users/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
        } else {
          if (!password) {
            toast("请设置初始密码", true);
            return;
          }
          await api("/api/users", { method: "POST", body: JSON.stringify(body) });
        }
        closeModal();
        toast("已保存");
        renderPage();
      } catch (err) {
        toast(err.message, true);
      }
    };
  }

  function resetUserPassword(id) {
    openModal(`
      <div class="modal-head"><h3>重置密码</h3></div>
      <form id="f" class="modal-body">
        <div class="field"><label>新密码</label>
          <input name="password" type="password" required minlength="6" autocomplete="new-password" /></div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">确定</button>
      </div>`);
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const pw = new FormData(e.target).get("password");
      try {
        await api(`/api/users/${id}/reset-password`, {
          method: "POST",
          body: JSON.stringify({ password: pw }),
        });
        closeModal();
        toast("密码已重置");
      } catch (err) {
        toast(err.message, true);
      }
    };
  }

  async function editRole(row) {
    let catalog = state.cache.permCatalog;
    if (!catalog) catalog = await api("/api/permissions");
    state.cache.permCatalog = catalog;
    const selected = new Set(row?.permissions || []);
    const groups = {};
    (catalog || []).forEach((p) => {
      (groups[p.group] || (groups[p.group] = [])).push(p);
    });
    const starOn = selected.has("*");
    openModal(
      `
      <div class="modal-head">
        <h3>${row ? "编辑角色" : "新建角色"}</h3>
        <p class="desc">勾选权限码；含 <code>*</code> 表示全部权限。</p>
      </div>
      <form id="f" class="modal-body">
        <div class="row">
          <div class="field"><label>名称</label>
            <input name="name" required value="${esc(row?.name || "")}" ${
              row?.is_system ? "readonly" : ""
            } /></div>
          <div class="field"><label>说明</label>
            <input name="description" value="${esc(row?.description || "")}" /></div>
        </div>
        <label class="check-row" style="margin-bottom:12px">
          <input type="checkbox" name="perm_star" id="perm-star" ${starOn ? "checked" : ""}/>
          <span><strong>全部权限 (*)</strong></span>
        </label>
        <div id="perm-list">${Object.keys(groups)
          .map(
            (g) => `<div class="seg" style="margin-bottom:12px">
            <div class="hint" style="margin:0 0 6px;font-weight:600">${esc(g)}</div>
            ${groups[g]
              .map(
                (p) => `<label class="check-row"><input type="checkbox" name="perms" value="${esc(
                  p.code
                )}" ${selected.has(p.code) ? "checked" : ""} ${
                  starOn ? "disabled" : ""
                }/> <span>${esc(p.label)} <code class="mono">${esc(p.code)}</code></span></label>`
              )
              .join("")}
          </div>`
          )
          .join("")}</div>
      </form>
      <div class="modal-actions">
        <button type="button" class="ghost" id="m-cancel">取消</button>
        <button class="primary" type="submit" form="f">保存</button>
      </div>`,
      { xl: true }
    );
    const star = document.getElementById("perm-star");
    const syncStar = () => {
      document.querySelectorAll('#perm-list input[name="perms"]').forEach((el) => {
        el.disabled = star.checked;
      });
    };
    star.onchange = syncStar;
    document.getElementById("m-cancel").onclick = closeModal;
    document.getElementById("f").onsubmit = async (e) => {
      e.preventDefault();
      const form = e.target;
      const fd = new FormData(form);
      let permissions = [];
      if (form.querySelector('[name="perm_star"]').checked) {
        permissions = ["*"];
      } else {
        permissions = [...form.querySelectorAll('[name="perms"]:checked')].map((x) => x.value);
      }
      const body = {
        name: String(fd.get("name") || "").trim(),
        description: String(fd.get("description") || "").trim(),
        permissions,
      };
      try {
        if (row) {
          await api(`/api/roles/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
        } else {
          await api("/api/roles", { method: "POST", body: JSON.stringify(body) });
        }
        closeModal();
        toast("已保存（相关用户需重新登录后权限生效）");
        renderPage();
      } catch (err) {
        toast(err.message, true);
      }
    };
  }

  async function editDepartment(row) {
    const depts = state.cache.departments || (await api("/api/departments"));
    state.cache.departments = depts;
    openModal(`
      <div class="modal-head">
        <h3>${row ? "编辑部门" : "新建部门"}</h3>
      </div>
      <form id="f" class="modal-body">
        <div class="field"><label>名称</label>
          <input name="name" required value="${esc(row?.name || "")}" /></div>
        <div class="field"><label>上级部门</label>
          <select name="parent_id">
            <option value="">— 无（根部门）—</option>
            ${depts
              .filter((d) => !row || d.id !== row.id)
              .map(
                (d) =>
                  `<option value="${esc(d.id)}" ${
                    row?.parent_id === d.id ? "selected" : ""
                  }>${esc(d.name)}</option>`
              )
              .join("")}
          </select>
        </div>
        <div class="field"><label>排序</label>
          <input name="sort_order" type="number" value="${row?.sort_order ?? 0}" /></div>
        <label class="check-row"><input type="checkbox" name="enabled" ${
          !row || row.enabled ? "checked" : ""
        }/> <span>启用</span></label>
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
      const body = {
        name: String(fd.get("name") || "").trim(),
        parent_id: String(fd.get("parent_id") || "") || null,
        sort_order: Number(fd.get("sort_order") || 0),
        enabled: form.querySelector('[name="enabled"]').checked,
      };
      try {
        if (row) {
          await api(`/api/departments/${row.id}`, { method: "PUT", body: JSON.stringify(body) });
        } else {
          await api("/api/departments", { method: "POST", body: JSON.stringify(body) });
        }
        closeModal();
        toast("已保存");
        renderPage();
      } catch (err) {
        toast(err.message, true);
      }
    };
  }

  tryBoot();
})();
