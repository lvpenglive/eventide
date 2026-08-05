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
