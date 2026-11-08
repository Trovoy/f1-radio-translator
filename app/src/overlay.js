import { getCurrentWindow } from "@tauri-apps/api/window";
import { $, ICON, escapeHtml, invoke, listen, speakerLabel } from "./shared.js";
import { providerName } from "./providers.js";

const win = getCurrentWindow();
const root = document.documentElement;
const linesEl = $("#lines");
const RM = matchMedia("(prefers-reduced-motion: reduce)").matches;
let style = { size: 34, font: "sans", align: "center", lines: 2, plate: 0, show_en: true };
let state = "idle";
const byId = new Map();

// ── hover: the toolbar shows while the pointer is anywhere over the window ──
let hoverTimer;
const hoverOn = () => { clearTimeout(hoverTimer); document.body.classList.add("hover"); };
const hoverOff = () => { clearTimeout(hoverTimer); hoverTimer = setTimeout(() => document.body.classList.remove("hover"), 380); };
document.documentElement.addEventListener("mouseenter", hoverOn);
document.addEventListener("pointermove", hoverOn);
// `pointerleave` never fires on `document`; the root element's mouseleave does when the pointer exits the window.
document.documentElement.addEventListener("mouseleave", hoverOff);
window.addEventListener("blur", hoverOff);

$("#grip").addEventListener("mousedown", (e) => { if (e.button === 0) win.startDragging(); });
$("#rs").addEventListener("mousedown", (e) => { if (e.button === 0) win.startResizeDragging("SouthEast"); });

// ── style ──
function applyStyle(s) {
  style = { ...style, ...s };
  root.style.setProperty("--sub", `${style.size}px`);
  root.style.setProperty("--plate", style.plate);
  const ov = $("#ov");
  ov.classList.toggle("serif", style.font === "serif");
  ov.classList.toggle("left", style.align === "left");
  ov.classList.toggle("one", style.lines === 1);
  ov.classList.toggle("zh-only", !style.show_en);
  $("#plate").setAttribute("aria-pressed", String(style.plate > 0));
  $("#en").setAttribute("aria-pressed", String(style.show_en));
}
const saveStyle = (patch) => { applyStyle(patch); invoke("update_style", { style }); };
$("#aUp").onclick = () => saveStyle({ size: Math.min(56, style.size + 3) });
$("#aDn").onclick = () => saveStyle({ size: Math.max(18, style.size - 3) });
$("#plate").onclick = () => saveStyle({ plate: style.plate > 0 ? 0 : 0.45 });
$("#en").onclick = () => saveStyle({ show_en: !style.show_en });

// ── lines ──
function lineEl(m, enter) {
  const d = document.createElement("div");
  d.className = "ln" + (enter && !RM ? " enter" : "") + (m.hint ? " hint" : "");
  d.innerHTML = `<div class="x"><div><div class="who">${escapeHtml(m.who)}</div></div></div>`
    + `<div class="zh">${m.pending ? '<span class="wait"><i></i><i></i><i></i></span>' : `<span>${escapeHtml(m.zh)}</span>`}</div>`
    + `<div class="x e"><div><div class="en">${escapeHtml(m.en ?? "")}</div></div></div>`;
  return d;
}

function push(m) {
  const cur = linesEl.querySelector(".ln:not(.old):not(.gone)");
  if (cur) cur.classList.add("old");
  [...linesEl.querySelectorAll(".ln.old:not(.gone)")].slice(0, -1).forEach((o) => {
    o.classList.add("gone");
    setTimeout(() => o.remove(), 450);
  });
  linesEl.querySelectorAll(".ln.hint").forEach((h) => h.remove());
  const el = lineEl(m, true);
  linesEl.appendChild(el);
  return el;
}

function resolve(el, zh, failed) {
  el.classList.toggle("failed", !!failed);
  el.querySelector(".zh").innerHTML = `<span class="got">${escapeHtml(zh)}</span>`;
}

listen("radio-line", ({ payload }) => {
  const el = push({ who: speakerLabel(payload.speaker), en: payload.text, pending: true });
  byId.set(payload.id, el);
  if (byId.size > 50) byId.delete(byId.keys().next().value);
});
listen("radio-translated", ({ payload }) => {
  const el = byId.get(payload.id);
  if (el) resolve(el, payload.zh);
});
listen("radio-failed", ({ payload }) => {
  const el = byId.get(payload.id);
  if (el) resolve(el, el.querySelector(".en")?.textContent || "（翻译失败）", true);
});

listen("overlay-demo", ({ payload }) => {
  const el = push({ who: speakerLabel(payload.speaker), en: payload.text, pending: true });
  setTimeout(() => resolve(el, payload.zh), 700);
});

// ── state ──
function setState(s, message) {
  state = s;
  $("#dot").className = "dot" + (s === "live" ? " live" : s === "paused" ? " paused" : "");
  $("#play").innerHTML = s === "live" ? ICON.pause : ICON.play;
  $("#play").setAttribute("aria-label", s === "live" ? "暂停" : "开始");
  $("#stop").innerHTML = ICON.stop;
  $("#stop").disabled = s === "idle";
  $("#ov").classList.toggle("paused", s === "paused");
  if (message) $("#stText").textContent = message;
  else $("#stText").textContent = s === "idle" ? "未开始" : s === "paused" ? "已暂停" : "正在翻译";
}
listen("radio-status", ({ payload }) => {
  const short = payload.level === "info" && payload.state === "live" ? null : payload.message;
  setState(payload.state, short);
  if (payload.level !== "info") toast(payload.message);
});
$("#play").onclick = () => invoke("engine_cmd", { action: "toggle" });
$("#stop").onclick = () => invoke("engine_cmd", { action: "stop" });
$("#settings").onclick = () => invoke("open_main", { page: "style" });
$("#lock").onclick = () => invoke("set_overlay_locked", { locked: true });

let toastTimer;
function toast(text) {
  const t = $("#toast");
  t.textContent = text;
  t.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("show"), 2800);
}
listen("overlay-locked", ({ payload }) => {
  document.body.classList.toggle("locked", payload);
  toast(payload ? "字幕窗已锁定，鼠标会穿透到视频 · Ctrl Alt L 解锁" : "已解锁，悬停可显示工具栏");
});
listen("style-changed", ({ payload }) => applyStyle(payload));
listen("settings-changed", ({ payload }) => {
  applyStyle(payload.style);
  if (state === "live") $("#stText").textContent = providerName(payload.chain[0]);
});

// ── boot ──
(async () => {
  const settings = await invoke("get_settings");
  applyStyle(settings.style);
  const snap = await invoke("get_state");
  setState(snap.state);
  document.body.classList.toggle("locked", snap.locked);
  const hint = lineEl({ who: "F1 Radio", zh: settings.region ? "已就绪，开始后字幕会显示在这里" : "请先在主窗口框选识别区域", hint: true }, true);
  linesEl.appendChild(hint);
  setTimeout(() => hint.isConnected && hint.classList.add("gone"), 6000);
  setTimeout(() => hint.isConnected && hint.remove(), 6500);
})();
