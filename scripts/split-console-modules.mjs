/**
 * One-shot splitter: extract api/ui/alerts modules from static/assets/app.js
 * and convert the remainder to an ES module that imports them.
 */
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, "../static/assets");
const APP = path.join(ROOT, "app.js");
const lines = fs.readFileSync(APP, "utf8").split(/\r?\n/);

function slice(start, end) {
  return lines.slice(start - 1, end).map((l) => (l.startsWith("  ") ? l.slice(2) : l));
}

function join(arr) {
  return arr.join("\n").replace(/\n+$/, "") + "\n";
}

const apiJs = `/* Eventide console — API client */
export const TOKEN_KEY = "eventide_token";
export const USER_KEY = "eventide_user";

let onUnauthorized = () => {};

/** Called on HTTP 401 (except login). Wired from app.js to logout(false). */
export function setUnauthorizedHandler(fn) {
  onUnauthorized = typeof fn === "function" ? fn : () => {};
}

export function token() {
  return localStorage.getItem(TOKEN_KEY);
}

export async function api(path, opts = {}) {
  const headers = Object.assign({}, opts.headers || {});
  if (!(opts.body instanceof FormData) && opts.body && !headers["Content-Type"]) {
    headers["Content-Type"] = "application/json";
  }
  const t = token();
  if (t) headers.Authorization = \`Bearer \${t}\`;
  const res = await fetch(path, { ...opts, headers });
  if (res.status === 401 && !path.includes("/api/auth/login")) {
    onUnauthorized();
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
  if (!res.ok) {
    const err = new Error((data && data.error) || text || res.statusText);
    if (data && data.code) err.code = data.code;
    if (res.status === 402 || data?.code === "license_readonly") {
      err.code = "license_readonly";
      err.message =
        (data && data.error) ||
        "许可证无效或已过期，当前为只读宽限。请到「系统设置」导入许可证。";
    }
    throw err;
  }
  return data;
}
`;

const severityBlock = join(slice(12, 31));
const uiHelpers = join(slice(183, 303));

const uiJs = `/* Eventide console — shared UI helpers */
${severityBlock}
${uiHelpers}
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
`;

let helpers = join(slice(4161, 4858));
helpers = helpers.replace(
  /function scheduleAlertsAutoRefresh\(tick\) \{[\s\S]*?\n\}/,
  `function scheduleAlertsAutoRefresh(tick) {
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
}`
);

let pageBody = join(slice(1672, 1868));
pageBody = pageBody
  .replace(/^\s*async alerts\(root\) \{/m, "async function renderAlerts(root) {")
  .replace(/,\s*$/, "");

const alertsJs = `/* Eventide console — alerts page + helpers */
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

${helpers}

${pageBody}

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
`;

fs.mkdirSync(path.join(ROOT, "js/pages"), { recursive: true });
fs.writeFileSync(path.join(ROOT, "js/api.js"), apiJs);
fs.writeFileSync(path.join(ROOT, "js/ui.js"), uiJs);
fs.writeFileSync(path.join(ROOT, "js/pages/alerts.js"), alertsJs);

const keep = [];
function pushRange(a, b) {
  for (let i = a; i <= b; i++) keep.push(lines[i - 1]);
}

keep.push("/* Eventide console SPA (ES modules) */");
keep.push('import { api, token, TOKEN_KEY, USER_KEY, setUnauthorizedHandler } from "./js/api.js";');
keep.push(
  "import {\n  severityOptions,\n  severityLabel,\n  toast,\n  hideCtxMenu,\n  showCtxMenu,\n  copyText,\n  esc,\n  cmpLabel,\n  fmtTime,\n  createModalApi,\n} from \"./js/ui.js\";"
);
keep.push('import { createAlertsModule, ALERT_PAGE_SIZE } from "./js/pages/alerts.js";');
keep.push("");

pushRange(5, 6);
pushRange(10, 11);
pushRange(33, 145);

pushRange(306, 563);

keep.push("  let alertsMod = null;");
keep.push("  function stopAlertsTimer() {");
keep.push("    if (alertsMod) alertsMod.stopAlertsTimer();");
keep.push("  }");
keep.push("");

pushRange(572, 615);

keep.push("  const { openModal, closeModal, modal, modalBody } = createModalApi();");
keep.push("  /** @returns {Promise<null|'merge'|'skip'>} */");
pushRange(634, 655);
keep.push("");

pushRange(660, 1671);

keep.push("    async alerts(root) {");
keep.push("      return alertsMod.renderAlerts(root);");
keep.push("    },");
keep.push("");

pushRange(1870, 4160);
pushRange(4860, 7893);

keep.push("");
keep.push("  alertsMod = createAlertsModule({");
keep.push("    api,");
keep.push("    state,");
keep.push("    toast,");
keep.push("    esc,");
keep.push("    fmtTime,");
keep.push("    severityOptions,");
keep.push("    severityLabel,");
keep.push("    navigate,");
keep.push("    setActions,");
keep.push("    openModal,");
keep.push("    closeModal,");
keep.push("    showCtxMenu,");
keep.push("    copyText,");
keep.push("    editSilence,");
keep.push("    showNotifyLogDetail,");
keep.push("  });");
keep.push("  const alertTable = (...args) => alertsMod.alertTable(...args);");
keep.push("  const bindAlertTable = (...args) => alertsMod.bindAlertTable(...args);");
keep.push("  const showAlertDetail = (...args) => alertsMod.showAlertDetail(...args);");
keep.push("  const showTrapRecentDetail = (...args) => alertsMod.showTrapRecentDetail(...args);");
keep.push("");
keep.push("  setUnauthorizedHandler(() => logout(false));");
keep.push("");
keep.push("  tryBoot();");
keep.push("");

let out = keep
  .join("\n")
  .split("\n")
  .map((l) => {
    if (l.startsWith("import ") || l.startsWith("/* Eventide") || l.startsWith("  severity") || l.startsWith("  toast") || l.startsWith("  hide") || l.startsWith("  show") || l.startsWith("  copy") || l.startsWith("  esc") || l.startsWith("  cmp") || l.startsWith("  fmt") || l.startsWith("  create") || l === "} from \"./js/ui.js\";") {
      return l;
    }
    // Keep multi-line import interior
    if (/^  (severityOptions|severityLabel|toast|hideCtxMenu|showCtxMenu|copyText|esc|cmpLabel|fmtTime|createModalApi),?$/.test(l)) {
      return l;
    }
    if (l.startsWith("  ")) return l.slice(2);
    return l;
  })
  .join("\n");

out = out.replace(/^\(\(\) => \{\s*/m, "");
out = out.replace(/\}\)\(\);\s*$/m, "");

const warnings = [];
if (out.includes("function severityOptions")) warnings.push("severityOptions still in app");
if (/async function api\s*\(/.test(out)) warnings.push("api still in app");
if (out.includes("function getAlertView")) warnings.push("getAlertView still in app");
if (out.includes("function toast(")) warnings.push("toast still in app");

fs.writeFileSync(APP, out.endsWith("\n") ? out : out + "\n");
console.log("Wrote", APP, "lines", out.split("\n").length);
console.log("Modules: js/api.js, js/ui.js, js/pages/alerts.js");
if (warnings.length) console.log("WARN:", warnings.join("; "));
else console.log("OK: extracted symbols not duplicated");
