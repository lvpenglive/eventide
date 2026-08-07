/* Eventide console — shared UI helpers */
// Zabbix-aligned severities (0–5)
const SEVERITIES = [
  ["not_classified", "未分类"],
  ["information", "信息"],
  ["warning", "警告"],
  ["average", "一般严重"],
  ["high", "严重"],
  ["disaster", "灾难"],
];
function severityOptions(selected, fallback = "warning") {
  const cur = selected || fallback;
  return SEVERITIES.map(
    ([v, label]) =>
      `<option value="${v}" ${cur === v ? "selected" : ""}>${label}</option>`
  ).join("");
}
function severityLabel(s) {
  const hit = SEVERITIES.find(([v]) => v === s);
  return hit ? hit[1] : s || "—";
}

function toast(msg, err = false) {
  const el = document.getElementById("toast");
  el.textContent = msg;
  el.classList.toggle("err", !!err);
  el.classList.add("show");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => el.classList.remove("show"), 2800);
}

let ctxMenuEl = null;
let ctxMenuCleanups = [];

function hideCtxMenu() {
  if (ctxMenuEl) {
    ctxMenuEl.remove();
    ctxMenuEl = null;
  }
  ctxMenuCleanups.forEach((fn) => {
    try {
      fn();
    } catch (_) {}
  });
  ctxMenuCleanups = [];
}

/** Floating right-click menu. items: { label, onClick, disabled? } | { sep: true } */
function showCtxMenu(clientX, clientY, items) {
  hideCtxMenu();
  const menu = document.createElement("div");
  menu.className = "ctx-menu";
  menu.setAttribute("role", "menu");
  (items || []).forEach((it) => {
    if (it && it.sep) {
      const hr = document.createElement("div");
      hr.className = "ctx-sep";
      menu.appendChild(hr);
      return;
    }
    if (!it || !it.label) return;
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "ctx-item" + (it.disabled ? " disabled" : "");
    btn.setAttribute("role", "menuitem");
    btn.textContent = it.label;
    btn.disabled = !!it.disabled;
    btn.onclick = (ev) => {
      ev.stopPropagation();
      if (btn.disabled) return;
      hideCtxMenu();
      try {
        it.onClick && it.onClick();
      } catch (err) {
        toast(err.message || String(err), true);
      }
    };
    menu.appendChild(btn);
  });
  document.body.appendChild(menu);
  ctxMenuEl = menu;

  const rect = menu.getBoundingClientRect();
  let left = clientX;
  let top = clientY;
  if (left + rect.width > window.innerWidth - 8) left = window.innerWidth - rect.width - 8;
  if (top + rect.height > window.innerHeight - 8) top = window.innerHeight - rect.height - 8;
  if (left < 8) left = 8;
  if (top < 8) top = 8;
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;

  const onDown = (ev) => {
    if (ctxMenuEl && !ctxMenuEl.contains(ev.target)) hideCtxMenu();
  };
  const onKey = (ev) => {
    if (ev.key === "Escape") hideCtxMenu();
  };
  const onScroll = () => hideCtxMenu();
  setTimeout(() => {
    document.addEventListener("mousedown", onDown, true);
    document.addEventListener("keydown", onKey, true);
    window.addEventListener("scroll", onScroll, true);
    ctxMenuCleanups.push(() => {
      document.removeEventListener("mousedown", onDown, true);
      document.removeEventListener("keydown", onKey, true);
      window.removeEventListener("scroll", onScroll, true);
    });
  }, 0);
}

async function copyText(text, label) {
  const t = String(text ?? "").trim();
  if (!t) {
    toast(`无可复制的${label || "内容"}`, true);
    return;
  }
  try {
    await navigator.clipboard.writeText(t);
    toast(`已复制${label || ""}`);
  } catch {
    toast("复制失败（浏览器权限或非 HTTPS）", true);
  }
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

function passwordToggleSvgs() {
  return `<svg class="eye-open" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">
      <path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/>
      <circle cx="12" cy="12" r="3"/>
    </svg>
    <svg class="eye-off" viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round" hidden>
      <path d="M2 12s3.5-7 10-7c2.1 0 3.9.6 5.4 1.5"/>
      <path d="M22 12s-3.5 7-10 7c-2.1 0-3.9-.6-5.4-1.5"/>
      <path d="M9.9 9.9a3 3 0 0 0 4.2 4.2"/>
      <path d="M3 3l18 18"/>
      <path d="M14.1 9.9 9.9 14.1"/>
    </svg>`;
}

/** Password input + eye toggle. Pass extra attrs like `required minlength="8"`. */
function passwordFieldHtml({
  name = "password",
  attrs = "",
  value = "",
  hint = "",
} = {}) {
  const valAttr = value ? ` value="${esc(value)}"` : "";
  const hintHtml =
    hint === false
      ? ""
      : hint
        ? `<div class="hint">${hint}</div>`
        : `<div class="hint">至少 8 位，须含大写、小写、数字与特殊字符</div>`;
  return `<div class="password-field">
    <input name="${esc(name)}" type="password"${valAttr} ${attrs} />
    <button type="button" class="password-toggle" data-pw-toggle aria-pressed="false" aria-label="显示密码" title="显示密码">
      ${passwordToggleSvgs()}
    </button>
  </div>${hintHtml}`;
}

/** Returns error message, or null if ok. Empty string is ok (caller decides required). */
function validatePasswordComplexity(password) {
  if (!password) return null;
  if ([...password].length < 8) return "密码至少 8 位";
  if (!/[a-z]/.test(password)) return "密码须包含小写字母";
  if (!/[A-Z]/.test(password)) return "密码须包含大写字母";
  if (!/[0-9]/.test(password)) return "密码须包含数字";
  if (!/[^A-Za-z0-9]/.test(password)) return "密码须包含特殊字符（如 !@#$%）";
  return null;
}

function bindPasswordToggles(root = document) {
  root.querySelectorAll("[data-pw-toggle]").forEach((btn) => {
    if (btn.dataset.bound === "1") return;
    btn.dataset.bound = "1";
    btn.addEventListener("click", () => {
      const input = btn.closest(".password-field")?.querySelector("input");
      if (!input) return;
      const show = input.type === "password";
      input.type = show ? "text" : "password";
      btn.setAttribute("aria-pressed", show ? "true" : "false");
      const label = show ? "隐藏密码" : "显示密码";
      btn.setAttribute("aria-label", label);
      btn.title = label;
      const open = btn.querySelector(".eye-open");
      const off = btn.querySelector(".eye-off");
      if (open) open.hidden = show;
      if (off) off.hidden = !show;
    });
  });
}

export {
  SEVERITIES,
  severityOptions,
  severityLabel,
  toast,
  hideCtxMenu,
  showCtxMenu,
  copyText,
  esc,
  cmpLabel,
  fmtTime,
  passwordFieldHtml,
  bindPasswordToggles,
  validatePasswordComplexity,
};

/** Modal helpers — call after DOM ready */
export function createModalApi() {
  const modal = document.getElementById("modal");
  const modalBody = document.getElementById("modal-body");
  function openModal(html, opts = {}) {
    modalBody.classList.remove("wide", "xl");
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
  modal.addEventListener("click", (e) => {
    if (e.target === modal) closeModal();
  });
  return { openModal, closeModal, modal, modalBody };
}
