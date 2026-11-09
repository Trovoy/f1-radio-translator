import { getCurrentWindow } from "@tauri-apps/api/window";
import { $, $$, ICON, escapeHtml, invoke, listen, mark, spaceMixed, speakerLabel, teamColor } from "./shared.js";
import { CATEGORIES, PROVIDER, PROVIDERS, fieldsFor, isConfigured, providerName } from "./providers.js";

const win = getCurrentWindow();
const root = document.documentElement;
const RM = matchMedia("(prefers-reduced-motion: reduce)").matches;

let settings = null;
let state = "idle";
let localStats = null; // built-in model speed: { running, stats, gpu }
let benching = false;
let ovVisible = true;
const lines = new Map(); // id -> { speaker, text, zh, ms, pending }
let queue = 0;

$("#appMark").innerHTML = mark({ draw: true });
// ── saving (debounced so typing a key does not write on every keystroke) ──
let saveTimer = null;
let lastSent = "";
// Key-sorted JSON: the backend's maps come back in arbitrary key order.
const canon = (v) => Array.isArray(v) ? `[${v.map(canon).join(",")}]`
  : v && typeof v === "object" ? `{${Object.keys(v).sort().map((k) => `${JSON.stringify(k)}:${canon(v[k])}`).join(",")}}`
  : JSON.stringify(v);
const signature = (s) => canon({ ...s, overlay: null, main_window: null });
function saveNow() {
  clearTimeout(saveTimer);
  saveTimer = null;
  lastSent = signature(settings);
  return invoke("save_settings", { settings });
}
function saveSoon() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, 450);
}

$("#wMin").onclick = () => win.minimize();
$("#wClose").onclick = async () => {
  if (saveTimer) await saveNow().catch(() => {});
  invoke("quit");
};

// ── navigation with sliding indicator + staggered page entrance ──
const ind = $("#ind");
let page = "home";
function placeInd() {
  const b = $('#nav [aria-current="true"]');
  if (!b) return;
  ind.style.width = `${b.offsetWidth}px`;
  ind.style.height = `${b.offsetHeight}px`;
  ind.style.transform = `translate(${b.offsetLeft}px, ${b.offsetTop}px)`;
}
function stagger(el) {
  [...el.children].forEach((c, i) => { c.classList.remove("stag"); void c.offsetWidth; c.style.setProperty("--i", i); c.classList.add("stag"); });
}
function go(p) {
  if (p === page || !$(`#p-${p}`)) return;
  const out = $(`#p-${page}`), inn = $(`#p-${p}`);
  $$("#nav button").forEach((b) => b.setAttribute("aria-current", String(b.dataset.p === p)));
  placeInd();
  const show = () => {
    out.hidden = true; out.classList.remove("leave");
    inn.hidden = false; inn.scrollTop = 0;
    if (!RM) stagger(inn);
    inn.querySelectorAll(".seg").forEach(placeThumb);
    if (p === "region") refreshPreview();
  };
  if (RM) show(); else { out.classList.add("leave"); setTimeout(show, 190); }
  page = p;
}
$("#nav").addEventListener("click", (e) => { const b = e.target.closest("button"); if (b) go(b.dataset.p); });

// ── collapsible sidebar (remembered on this computer) ──
const winEl = $("#win");
function setMini(on, animate = true) {
  if (!animate) winEl.classList.add("no-anim");
  winEl.classList.toggle("mini", on);
  const label = on ? "展开侧栏" : "收起侧栏";
  $("#sideTg").title = label; $("#sideTg").setAttribute("aria-label", label);
  try { localStorage.setItem("side-mini", on ? "1" : "0"); } catch {}
  if (!animate) requestAnimationFrame(() => requestAnimationFrame(() => winEl.classList.remove("no-anim")));
}
try { if (localStorage.getItem("side-mini") === "1") setMini(true, false); } catch {}
$("#sideTg").onclick = () => setMini(!winEl.classList.contains("mini"));
// The active-page highlight follows the buttons as the rail widens or narrows.
new ResizeObserver(placeInd).observe($("#nav"));
listen("navigate", ({ payload }) => go(payload));

// ── segmented controls ──
function placeThumb(seg) {
  const th = seg.querySelector(".thumb"), b = seg.querySelector('[aria-pressed="true"]');
  if (!th || !b) return;
  th.style.width = `${b.offsetWidth}px`;
  th.style.transform = `translateX(${b.offsetLeft}px)`;
}
function seg(id, onPick) {
  const s = $(id);
  const th = document.createElement("span");
  th.className = "thumb";
  s.prepend(th);
  s.addEventListener("click", (e) => {
    const b = e.target.closest("button");
    if (!b) return;
    setSeg(id, b.dataset.v ?? b.dataset.c);
    onPick?.(b);
  });
}
function setSeg(id, value) {
  const s = $(id);
  s.querySelectorAll("button").forEach((x) => x.setAttribute("aria-pressed", String((x.dataset.v ?? x.dataset.c) === String(value))));
  placeThumb(s);
}
const toggle = (el, on) => el.setAttribute("aria-checked", String(!!on));
const isOn = (el) => el.getAttribute("aria-checked") === "true";
function fillRange(r) { r.style.setProperty("--p", `${((r.value - r.min) / (r.max - r.min)) * 100}%`); }

// ── home ──
function morph(btn, apply) {
  const w0 = btn.offsetWidth;
  btn.style.width = "";
  apply();
  const w1 = btn.offsetWidth;
  btn.style.width = `${w0}px`;
  void btn.offsetWidth;
  btn.style.width = `${w1}px`;
}

function count(el, to, dec = 0, suffix = "") {
  const from = parseFloat(el.dataset.v || "0");
  el.dataset.v = to;
  if (RM) { el.innerHTML = to.toFixed(dec) + suffix; return; }
  const t0 = performance.now();
  const step = (t) => {
    const p = Math.max(0, Math.min(1, (t - t0) / 600)), e = 1 - Math.pow(1 - p, 3);
    el.innerHTML = (from + (to - from) * e).toFixed(dec) + suffix;
    if (p < 1) requestAnimationFrame(step);
  };
  requestAnimationFrame(step);
}

function stats() {
  const done = [...lines.values()].filter((l) => l.zh && !l.failed);
  count($("#nCnt"), done.length);
  const recent = done.slice(-20);
  const avg = recent.length ? recent.reduce((a, b) => a + b.ms, 0) / recent.length / 1000 : 0;
  count($("#nLat"), avg, 2, "<small>秒</small>");
  count($("#nQ"), queue);
}

function homeSub() {
  if (!settings) return;
  const r = settings.region;
  $("#hSub").textContent = [
    providerName(settings.chain[0]) + (settings.chain[0] === "local" && localStats?.stats?.gen_tps ? ` · ${Math.round(localStats.stats.gen_tps)} tok/s` : ""),
    r ? `识别区域 ${r.w} × ${r.h}` : "尚未框选识别区域",
    ovVisible ? "字幕窗已显示" : "字幕窗已隐藏",
  ].join(" · ");
}

let clock = null, secs = 0;
function setState(s) {
  const prev = state;
  state = s;
  const live = s === "live";
  $("#sideDot").className = "dot" + (live ? " live" : s === "paused" ? " paused" : "");
  $("#sideSt").textContent = { idle: "未开始", live: "翻译中", paused: "已暂停" }[s];
  $("#win").classList.toggle("live-on", live);
  const t = $("#hTitle");
  const title = { idle: "准备就绪", live: "正在翻译", paused: "已暂停" }[s];
  if (t.textContent !== title) {
    t.textContent = title;
    t.classList.remove("swap"); void t.offsetWidth; t.classList.add("swap");
  }
  const g = $("#hGo");
  morph(g, () => {
    g.classList.toggle("live", s !== "idle");
    $("#hIc").innerHTML = live ? ICON.pause : ICON.play;
    const lb = $("#hLb");
    lb.textContent = live ? "暂停" : s === "paused" ? "继续" : "开始翻译";
    lb.classList.remove("swap"); void lb.offsetWidth; lb.classList.add("swap");
  });
  $("#hStop").classList.toggle("off", s === "idle");
  if (s === "idle") { clearInterval(clock); clock = null; secs = 0; $("#sideTime").textContent = "00:00"; queue = 0; stats(); }
  else if (prev === "idle" && !clock) {
    // A new session counts from zero.
    lines.clear();
    queue = 0;
    stats();
    clock = setInterval(() => {
      if (state !== "live") return;
      secs++;
      $("#sideTime").textContent = `${String((secs / 60) | 0).padStart(2, "0")}:${String(secs % 60).padStart(2, "0")}`;
    }, 1000);
  }
}
$("#hStop").innerHTML = ICON.stop;
$("#hGo").onclick = () => invoke("engine_cmd", { action: "toggle" });
$("#hStop").onclick = () => invoke("engine_cmd", { action: "stop" });

function showMsg(level, text) {
  const m = $("#hMsg");
  if (level === "info") { m.hidden = true; return; }
  m.className = `msg ${level}`;
  m.textContent = text;
  m.hidden = false;
}

listen("radio-status", ({ payload }) => {
  setState(payload.state);
  showMsg(payload.level, payload.message);
  if (payload.level === "info" && payload.message.startsWith("正在识别")) $("#ocrText").textContent = payload.message.replace("正在识别 · ", "Windows 文字识别 · ");
});
listen("radio-line", ({ payload }) => {
  lines.set(payload.id, { ...lines.get(payload.id), ...payload, pending: true });
  queue++;
  stats();
});
listen("radio-translated", ({ payload }) => {
  const l = lines.get(payload.id);
  if (!l) return;
  Object.assign(l, { zh: payload.zh, ms: payload.ms, pending: false });
  provMs[payload.provider] = payload.ms;
  if (settings) { layoutTopo(false); pulseTopo(payload.provider); }
  queue = Math.max(0, queue - 1);
  stats();
  addRecent(l);
  if (lines.size > 300) lines.delete(lines.keys().next().value);
});
listen("radio-failed", ({ payload }) => {
  const l = lines.get(payload.id);
  if (l) Object.assign(l, { failed: true, pending: false });
  queue = Math.max(0, queue - 1);
  stats();
});

function addRecent(l) {
  const ul = $("#recent");
  ul.querySelector(".none")?.remove();
  const li = document.createElement("li");
  li.className = "in";
  li.innerHTML = `<div class="m" style="--team:${teamColor(l.speaker)}">${escapeHtml(speakerLabel(l.speaker))}</div><div class="z">${escapeHtml(spaceMixed(l.zh))}</div>`;
  ul.prepend(li);
  [...ul.children].slice(3).forEach((x) => x.remove());
}

// overlay switches
$("#ovShow").onclick = () => invoke("set_overlay_visible", { visible: !isOn($("#ovShow")) });
$("#ovLock").onclick = () => invoke("set_overlay_locked", { locked: !isOn($("#ovLock")) });
listen("overlay-visible", ({ payload }) => { ovVisible = payload; toggle($("#ovShow"), payload); homeSub(); });
listen("overlay-locked", ({ payload }) => toggle($("#ovLock"), payload));

// waveform: rises with the live state, settles to a calm line otherwise
const wc = $("#wave"), wg = wc.getContext("2d");
let amp = 0, ph = 0;
function wave() {
  const W = wc.width, H = wc.height, target = state === "live" ? 1 : state === "paused" ? 0.15 : 0.04;
  amp += (target - amp) * 0.05;
  ph += 0.035;
  wg.clearRect(0, 0, W, H);
  [[0.9, 1.6, 0.55], [0.6, 2.3, 0.3], [0.35, 3.1, 0.18]].forEach(([a, f, al], j) => {
    wg.beginPath();
    for (let x = 0; x <= W; x += 6) {
      const t = x / W, env = Math.sin(t * Math.PI);
      const y = H / 2 + Math.sin(t * Math.PI * 2 * f + ph * (1 + j * 0.4)) * Math.sin(t * Math.PI * 3.1 - ph * 0.6) * env * a * amp * (H * 0.42);
      x ? wg.lineTo(x, y) : wg.moveTo(x, y);
    }
    wg.strokeStyle = `rgba(244,242,238,${al})`;
    wg.lineWidth = 2;
    wg.stroke();
  });
  if (!RM) requestAnimationFrame(wave);
}
wave();

// ── services ──
let cat = "mt", open = null;
const CK = `<svg class="ck" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7"/></svg>`;

// ── service order as a live topology: transcript → services, top to bottom → subtitle window ──
const topo = $("#chain");
const T = { row: 58, h: 46, w: 236 };
const provMs = {};   // provider id -> latency of its last translated line
const TIC = {
  in: '<svg class="ic" viewBox="0 0 24 24"><path d="M8.6 8.6a4.8 4.8 0 0 0 0 6.8M15.4 8.6a4.8 4.8 0 0 1 0 6.8M5.6 5.6a9 9 0 0 0 0 12.8M18.4 5.6a9 9 0 0 1 0 12.8"/><path class="r" d="M12 12h.01"/></svg>',
  out: '<svg class="ic" viewBox="0 0 24 24"><path d="M7 4.5h10a3.5 3.5 0 0 1 3.5 3.5v8a3.5 3.5 0 0 1-3.5 3.5H7A3.5 3.5 0 0 1 3.5 16V8A3.5 3.5 0 0 1 7 4.5Z"/><path d="M10.5 15.5h6M8.5 12.2h7"/><path class="r" d="M7.6 15.5h.01"/></svg>',
  grip: '<svg viewBox="0 0 10 16" fill="currentColor"><circle cx="3" cy="4" r="1"/><circle cx="7" cy="4" r="1"/><circle cx="3" cy="8" r="1"/><circle cx="7" cy="8" r="1"/><circle cx="3" cy="12" r="1"/><circle cx="7" cy="12" r="1"/></svg>',
};
let tdrag = null;    // { id, from, to, y0, dy, moved, node }

function topoSub(id, i) {
  if (!isConfigured(settings, id)) return '<span class="warn">未配置</span>';
  if (provMs[id] != null) return `最近 ${(provMs[id] / 1000).toFixed(2)} 秒`;
  return i === 0 ? "优先使用" : "主用失败时接手";
}

function renderChain(fresh) {
  const n = settings.chain.length;
  topo.style.height = `${Math.max(150, (n - 1) * T.row + T.h + 52)}px`;
  topo.innerHTML = '<svg class="topo-lines" aria-hidden="true"></svg>'
    + `<div class="tio in">${TIC.in}<span>转录</span></div><div class="tio out">${TIC.out}<span>字幕窗</span></div>`
    + '<span class="tlat" hidden></span>'
    + settings.chain.map((id) => `<div class="tn${id === fresh ? " fresh" : ""}" data-id="${id}">`
      + `<span class="tg-grip">${TIC.grip}</span><span class="tt"><b>${escapeHtml(providerName(id))}</b><small></small></span><span class="role"></span>`
      + (n > 1 ? `<button data-rm="${id}" title="移出顺序" aria-label="移出">✕</button>` : "") + "</div>").join("");
  layoutTopo(false);
  homeSub();
  renderHomeLocal();
}

/** Places the nodes for the current (or dragged) order, then redraws the lines as they settle. */
function layoutTopo(animate = true) {
  const n = settings.chain.length, H = parseFloat(topo.style.height);
  const top0 = (H - ((n - 1) * T.row + T.h)) / 2;
  const order = [...settings.chain];
  if (tdrag) { order.splice(tdrag.from, 1); order.splice(tdrag.to, 0, tdrag.id); }
  topo.querySelectorAll(".tn").forEach((el) => {
    const id = el.dataset.id, slot = order.indexOf(id);
    const y = tdrag?.id === id ? top0 + tdrag.from * T.row + tdrag.dy : top0 + slot * T.row;
    if (!animate) el.style.transition = "none";
    el.style.transform = `translateY(${y}px)`;
    if (!animate) { void el.offsetWidth; el.style.transition = ""; }
    el.classList.toggle("pri", slot === 0);
    el.querySelector(".role").textContent = slot === 0 ? "主用" : `备用 ${slot}`;
    el.querySelector("small").innerHTML = topoSub(id, slot);
  });
  followLines(animate ? 480 : 0);
}

let linesUntil = 0, linesRaf = 0;
function followLines(ms) {
  linesUntil = Math.max(linesUntil, performance.now() + ms);
  if (linesRaf) return;
  const tick = () => { drawLines(); linesRaf = performance.now() < linesUntil || tdrag ? requestAnimationFrame(tick) : 0; };
  tick();
}

function drawLines() {
  const svg = topo.querySelector(".topo-lines");
  if (!svg || !topo.offsetParent) return;
  const b = topo.getBoundingClientRect();
  const port = (el, side) => { const r = el.getBoundingClientRect(); return [side === "l" ? r.left - b.left : r.right - b.left, r.top - b.top + r.height / 2]; };
  const [ix, iy] = port(topo.querySelector(".tio.in"), "r"), [ox, oy] = port(topo.querySelector(".tio.out"), "l");
  const curve = ([x1, y1], [x2, y2]) => { const d = (x2 - x1) * .55; return `M${x1} ${y1}C${x1 + d} ${y1} ${x2 - d} ${y2} ${x2} ${y2}`; };
  let out = "";
  let pri = null;
  topo.querySelectorAll(".tn").forEach((el) => {
    const id = el.dataset.id, cls = el.classList.contains("pri") ? "act" : "bk";
    const a = curve([ix + 6, iy], port(el, "l")), z = curve(port(el, "r"), [ox - 6, oy]);
    if (cls === "act") pri = { id, z };
    for (const [edge, d] of [["i", a], ["o", z]]) {
      out += `<path class="base" d="${d}"/><path class="flow ${cls}" data-e="${edge}-${id}" d="${d}"/>`;
    }
  });
  svg.innerHTML = out;
  // Latency of the primary service, on its outgoing line.
  const lat = topo.querySelector(".tlat");
  const ms = pri && provMs[pri.id];
  lat.hidden = !ms;
  if (ms) {
    const p = svg.querySelector(`[data-e="o-${pri.id}"]`), m = p.getPointAtLength(p.getTotalLength() / 2);
    lat.textContent = `${(ms / 1000).toFixed(2)} 秒`;
    lat.style.left = `${m.x}px`; lat.style.top = `${m.y}px`;
  }
}

/** A light travels the route a translated line actually took. */
function pulseTopo(id) {
  if (RM || !topo.offsetParent || tdrag) return;
  ["i", "o"].forEach((edge, k) => {
    const p = topo.querySelector(`.topo-lines [data-e="${edge}-${id}"]`);
    if (!p) return;
    const len = p.getTotalLength(), c = p.cloneNode();
    c.setAttribute("class", "pulse");
    p.parentNode.appendChild(c);
    c.animate([{ strokeDasharray: `28 ${len}`, strokeDashoffset: 28 }, { strokeDasharray: `28 ${len}`, strokeDashoffset: -len }],
      { duration: 650, delay: k * 520, easing: "cubic-bezier(.4, 0, .2, 1)", fill: "both" }).onfinish = () => c.remove();
  });
  const node = topo.querySelector(`.tn[data-id="${id}"]`);
  if (node) setTimeout(() => { node.classList.remove("hit"); void node.offsetWidth; node.classList.add("hit"); }, 480);
}

topo.addEventListener("pointerdown", (e) => {
  const node = e.target.closest(".tn");
  if (!node || e.button !== 0 || e.target.closest("[data-rm]")) return;
  const from = settings.chain.indexOf(node.dataset.id);
  tdrag = { id: node.dataset.id, from, to: from, y0: e.clientY, dy: 0, moved: false, node };
  node.setPointerCapture(e.pointerId);
});
topo.addEventListener("pointermove", (e) => {
  if (!tdrag) return;
  const n = settings.chain.length, dy = e.clientY - tdrag.y0;
  if (!tdrag.moved && Math.abs(dy) < 4) return;
  if (!tdrag.moved) { tdrag.moved = true; tdrag.node.classList.add("drag"); topo.classList.add("dragging"); }
  const min = -tdrag.from * T.row - 10, max = (n - 1 - tdrag.from) * T.row + 10;
  tdrag.dy = Math.max(min, Math.min(max, dy));
  tdrag.to = Math.max(0, Math.min(n - 1, Math.round(tdrag.from + tdrag.dy / T.row)));
  layoutTopo();
});
const endDrag = () => {
  if (!tdrag) return;
  const d = tdrag; tdrag = null;
  d.node.classList.remove("drag"); topo.classList.remove("dragging");
  if (!d.moved) { showProvider(d.id); return; }
  if (d.to !== d.from) {
    const order = [...settings.chain]; order.splice(d.from, 1); order.splice(d.to, 0, d.id);
    settings.chain = order;
    saveSoon(); renderList(false);
  }
  layoutTopo();
};
topo.addEventListener("pointerup", endDrag);
topo.addEventListener("pointercancel", endDrag);
new ResizeObserver(() => followLines(0)).observe(topo);

/** Clicking a node opens that service's settings in the list below. */
function showProvider(id) {
  const c = PROVIDER[id]?.c;
  if (!c) return;
  if (c !== cat) $(`#catSeg button[data-c="${c}"]`)?.click();
  open = id;
  renderList(false);
  requestAnimationFrame(() => $(`#plist .pv[data-id="${id}"]`)?.scrollIntoView({ behavior: RM ? "auto" : "smooth", block: "center" }));
}

topo.addEventListener("click", (e) => {
  const id = e.target.closest("[data-rm]")?.dataset.rm;
  if (!id || settings.chain.length < 2) return;
  settings.chain = settings.chain.filter((x) => x !== id);
  renderChain(); renderList(false); saveSoon();
});

function renderList(anim) {
  $("#plist").innerHTML = PROVIDERS.filter((p) => p.c === cat).map((p, i) => {
    const r = settings.chain.indexOf(p.id);
    const v = fieldsFor(settings, p.id);
    const status = r === 0 ? "主用" : r > 0 ? `备用 ${r}` : isConfigured(settings, p.id) && p.f.length ? "已配置" : "";
    const fields = p.f.map((f, j) => {
      const id = `f_${p.id}_${f.k}`;
      return `<div class="fld"><label for="${id}">${escapeHtml(f.l)}</label><div class="inp"${j === p.f.length - 1 ? ` id="inp_${p.id}"` : ""}>`
        + `<input id="${id}" data-p="${p.id}" data-k="${f.k}" ${f.secret ? 'type="password"' : ""} value="${escapeHtml(v[f.k])}" placeholder="${f.secret ? "粘贴密钥" : escapeHtml(f.def ?? "")}" autocomplete="off" spellcheck="false">`
        + `${f.secret ? `<button type="button" data-eye="${id}">显示</button>` : ""}<span class="bar"></span></div></div>`;
    }).join("");
    const actions = (r === 0 ? '<button class="btn" disabled>当前主用</button>' : `<button class="btn pri" data-pri="${p.id}">设为主用</button>`)
      + (r < 0 ? `<button class="btn" data-add="${p.id}">加入备用</button>` : "")
      + `<button class="btn" data-test="${p.id}">测试</button><span class="res" id="res_${p.id}"></span>`;
    return `<div class="pv${r === 0 ? " primary" : ""}${open === p.id ? " open" : ""}${anim && !RM ? " stag" : ""}" style="--i:${i}" data-id="${p.id}">`
      + `<button aria-expanded="${open === p.id}"><span><span class="pn">${escapeHtml(p.n)}</span><span class="pd">${escapeHtml(p.d)}</span></span><span class="ps${status ? " on" : ""}">${status}</span>${CK}</button>`
      + `<div class="det"><div><div class="in">${p.custom ? `<div id="lmWrap">${localPanelHTML()}</div>` : fields}<div class="acts">${actions}</div></div></div></div></div>`;
  }).join("");
}

/** Writes the shown values, defaults included, into settings so the backend sees the same thing as the page. */
function persistFields(id) {
  settings.providers[id] = { ...fieldsFor(settings, id), ...settings.providers[id] };
}

$("#plist").addEventListener("input", (e) => {
  const t = e.target;
  if (!t.dataset.p) return;
  persistFields(t.dataset.p);
  settings.providers[t.dataset.p][t.dataset.k] = t.value;
  saveSoon();
});

$("#plist").addEventListener("click", async (e) => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.dataset.eye) {
    const i = document.getElementById(t.dataset.eye), show = i.type === "password";
    i.type = show ? "text" : "password"; t.textContent = show ? "隐藏" : "显示";
    return;
  }
  if (t.dataset.pri) {
    const id = t.dataset.pri;
    persistFields(id);
    settings.chain = [id, ...settings.chain.filter((x) => x !== id)];
    renderChain(id);
    $$("#plist .pv").forEach((p) => p.classList.toggle("primary", p.dataset.id === id));
    setTimeout(() => renderList(false), 480);
    saveSoon();
    return;
  }
  if (t.dataset.add) {
    persistFields(t.dataset.add);
    settings.chain = [...settings.chain, t.dataset.add];
    renderChain(t.dataset.add); renderList(false); saveSoon();
    return;
  }
  if (t.dataset.test) {
    const id = t.dataset.test, box = $(`#inp_${id}`), res = $(`#res_${id}`);
    res.className = "res"; res.textContent = "";
    box?.classList.remove("testing"); void box?.offsetWidth; box?.classList.add("testing");
    t.disabled = true;
    try {
      const r = await invoke("test_provider", { id, fields: PROVIDER[id]?.custom ? (settings.providers[id] ?? {}) : fieldsFor(settings, id) });
      res.innerHTML = `${ICON.check}可用 · ${(r.ms / 1000).toFixed(2)} 秒 · ${escapeHtml(r.text)}`;
    } catch (err) {
      res.className = "res err";
      res.textContent = String(err);
    } finally {
      box?.classList.remove("testing");
      t.disabled = false;
    }
    return;
  }
  const head = t.closest(".pv > button");
  if (head) {
    const pv = head.parentElement, was = pv.classList.contains("open");
    $$("#plist .pv.open").forEach((x) => { x.classList.remove("open"); x.querySelector("button").setAttribute("aria-expanded", "false"); });
    if (!was) { pv.classList.add("open"); head.setAttribute("aria-expanded", "true"); }
    open = was ? null : pv.dataset.id;
  }
});

seg("#catSeg", (b) => { cat = b.dataset.c; open = null; renderList(true); });

// ── built-in local models (downloaded on demand, run on the GPU) ──
let localInfo = null;
const localProg = {};   // model id -> latest progress event
const localErr = {};    // model id -> last download error
let confirmDelete = null;
const gb = (b) => `${(b / 1024 ** 3).toFixed(1)} GB`;
const mb = (b) => `${Math.round(b / 1024 ** 2)} MB`;
const SMALL_CK = '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7"/></svg>';

function progressText(p) {
  if (!p) return "准备下载…";
  if (p.task === "unpack") return p.label;
  if (p.task === "move") return `正在移动 ${p.total ? Math.floor((p.done / p.total) * 100) : 0}% · ${gb(p.done)} / ${gb(p.total)}`;
  const pct = p.total ? Math.floor((p.done / p.total) * 100) : 0;
  const speed = p.speed > 0 ? ` · ${(p.speed / 1024 ** 2).toFixed(1)} MB/s` : "";
  return `${p.label} ${pct}%${speed}`;
}

function localPanelHTML() {
  if (!localInfo) return '<div class="lm-engine">正在读取本地模型…</div>';
  const using = settings?.providers?.local?.model || "";
  const kind = localInfo.engine_kind === "cuda" ? "CUDA 版（NVIDIA 显卡）" : "Vulkan 版（通用显卡）";
  const engine = localInfo.engine_installed
    ? `推理引擎 · ${kind} · 已安装`
    : `推理引擎 · ${kind} · 首次下载模型时会一并下载（约 ${mb(localInfo.engine_size)}）`;
  const cards = localInfo.models.map((m) => {
    const busy = localInfo.downloading === m.id;
    const moving = localInfo.downloading === "__move";
    const p = localProg[m.id];
    let act;
    if (busy) {
      const pct = p?.total ? (p.done / p.total) * 100 : 0;
      act = `<div class="lm-prog"><div class="lm-bar"><i style="width:${pct}%"></i></div><span class="lm-pct">${escapeHtml(progressText(p))}</span><button class="btn ghost" data-lcancel>取消</button></div>`;
    } else if (!m.installed) {
      act = `<button class="btn" data-ldl="${m.id}"${localInfo.downloading ? " disabled" : ""}>下载</button>`;
    } else if (moving) {
      act = '<span class="lm-meta">移动中…</span>';
    } else {
      act = (using === m.id ? `<span class="lm-using">${SMALL_CK}正在使用</span>` : `<button class="btn pri" data-luse="${m.id}">使用此模型</button>`)
        + `<button class="btn ghost${confirmDelete === m.id ? " danger" : ""}" data-ldel="${m.id}">${confirmDelete === m.id ? "确认删除" : "删除"}</button>`;
    }
    const tags = `<span class="tag">${escapeHtml(m.family)}</span>${m.recommended ? '<span class="tag rec">推荐</span>' : ""}${localInfo.running === m.id ? '<span class="tag run">运行中</span>' : ""}`;
    return `<div class="lm${using === m.id ? " using" : ""}" data-m="${m.id}">`
      + `<div class="lm-top"><span class="lm-name">${escapeHtml(m.name)}</span>${tags}</div>`
      + `<div class="lm-desc">${escapeHtml(m.desc)}</div>`
      + `<div class="lm-foot"><span class="lm-meta">${gb(m.size)} · 约 ${m.vram_gb} GB 显存</span><div class="lm-act">${act}</div></div>`
      + (localErr[m.id] ? `<div class="lm-err">${escapeHtml(localErr[m.id])}</div>` : "")
      + `</div>`;
  }).join("");
  return `${dirHTML()}<div class="lm-engine">${engine}</div>${perfHTML(using)}<div class="lm-list">${cards}</div>`;
}

// ── where the engine and models live ──
let dirErr = "";
function dirHTML() {
  const moving = localInfo.downloading === "__move";
  const p = localProg.__move;
  const free = localInfo.dir_free != null ? ` · 剩余 ${gb(localInfo.dir_free)}` : "";
  const act = moving
    ? `<div class="lm-prog"><div class="lm-bar"><i style="width:${p?.total ? (p.done / p.total) * 100 : 0}%"></i></div><span class="lm-pct">${escapeHtml(p ? progressText(p) : "正在移动…")}</span></div>`
    : `<button class="btn ghost" data-ldir${localInfo.downloading ? " disabled" : ""}>更改</button>`
      + (localInfo.dir_is_default ? "" : `<button class="btn ghost" data-ldirreset${localInfo.downloading ? " disabled" : ""}>恢复默认</button>`);
  return `<div class="lm-dir"><div class="lm-dir-t"><span class="lm-dir-l">存放位置</span>`
    + `<span class="lm-dir-p" title="${escapeHtml(localInfo.dir)}">${escapeHtml(localInfo.dir)}</span><span class="lm-dir-f">${free}</span></div>`
    + `<div class="lm-act">${act}</div></div>`
    + (dirErr ? `<div class="lm-err">${escapeHtml(dirErr)}</div>` : "");
}

async function relocate(dir) {
  dirErr = "";
  delete localProg.__move;
  localInfo.downloading = "__move";
  renderLocalPanel();
  try { await invoke("local_relocate", { dir }); }
  catch (err) { dirErr = `没有移动成功：${err}`; }
  loadLocal();
}

// ── live speed of the loaded model ──

function perfHTML(using, home = false) {
  const m = localInfo?.models.find((x) => x.id === using && x.installed);
  if (!m) return "";
  return `<div class="lm-perf perfbox${home ? " home" : ""}">`
    + `<div class="lm-perf-head"><span class="lm-perf-t">${home ? escapeHtml(m.name) : "运行状态"}</span><span class="lm-perf-sub" data-p="sub"></span>`
    + `<button class="btn ghost" data-lbench="${m.id}"${benching || localInfo.downloading ? " disabled" : ""}>${benching ? "测速中…" : "测速"}</button></div>`
    + `<div class="lm-stats">`
    + `<div><b data-p="tps">–</b><span>生成速度</span></div>`
    + `<div><b data-p="lat">–</b><span>单句耗时</span></div>`
    + `<div><b data-p="first">–</b><span>首字延迟</span></div>`
    + `<div><b data-p="vram">–</b><span data-p="vramL">显存</span></div>`
    + `</div><svg class="lm-spark" viewBox="0 0 300 28" preserveAspectRatio="none"><path data-p="spark"/></svg></div>`;
}

/** The overview shows the same block while the built-in model is the primary service. */
function renderHomeLocal() {
  const box = $("#hLocal");
  if (!box || !settings) return;
  const using = settings.providers?.local?.model;
  const html = settings.chain[0] === "local" && using ? perfHTML(using, true) : "";
  box.hidden = !html;
  box.querySelector(".hl-body").innerHTML = html;
  paintPerf();
  pollPerf();
}

function paintPerf() {
  const boxes = document.querySelectorAll(".perfbox");
  if (!boxes.length) return;
  const r = localStats, s = r?.stats;
  const using = settings?.providers?.local?.model;
  const fresh = s && s.model === using;
  const name = localInfo?.models.find((x) => x.id === using)?.name ?? "";
  const loaded = r?.running === using;
  const detail = [fresh && s.load_ms ? `用时 ${(s.load_ms / 1000).toFixed(1)} 秒` : "", fresh && s.calls ? `已翻译 ${s.calls} 句` : ""].filter(Boolean);
  const has = fresh && s.calls > 0;
  const g = r?.gpu;
  // Latency of the last 30 lines; the line rests at the bottom until there is data.
  const pts = has ? s.recent : [];
  let d = "M0 27 H300";
  if (pts.length > 1) {
    const max = Math.max(...pts) * 1.15, step = 300 / (pts.length - 1);
    d = pts.map((v, i) => `${i ? "L" : "M"}${(i * step).toFixed(1)} ${(27 - (v / max) * 24).toFixed(1)}`).join(" ");
  }
  for (const box of boxes) {
    const q = (k) => box.querySelector(`[data-p="${k}"]`);
    const home = box.classList.contains("home");
    q("sub").textContent = loaded
      ? [home ? "已加载" : `${name} 已加载`, ...detail].join(" · ")
      : has ? "未加载 · 以下是上次运行的数据" : "未加载 · 开始翻译或点测速时自动加载";
    q("tps").innerHTML = has && s.gen_tps ? `${Math.round(s.gen_tps)}<small>tok/s</small>` : "–";
    q("lat").innerHTML = has ? `${(s.avg_ms / 1000).toFixed(2)}<small>秒</small>` : "–";
    q("first").innerHTML = has && s.first_ms ? `${s.first_ms}<small>毫秒</small>` : "–";
    q("vram").innerHTML = g ? `${(g.used_mb / 1024).toFixed(1)}<small>/ ${Math.round(g.total_mb / 1024)} GB</small>` : "–";
    q("vramL").textContent = g ? `显存 · GPU ${g.util}%` : "显存";
    q("spark").setAttribute("d", d);
  }
}

async function pollPerf() {
  // Only while a speed block is on screen.
  if (![...document.querySelectorAll(".perfbox")].some((b) => b.offsetParent)) return;
  try { localStats = await invoke("local_stats"); paintPerf(); } catch {}
}
setInterval(pollPerf, 2000);

listen("local-stats", ({ payload }) => {
  localStats = { ...(localStats ?? {}), stats: payload };
  paintPerf();
  homeSub();
});

function renderLocalPanel() {
  const w = $("#lmWrap");
  if (w) w.innerHTML = localPanelHTML();
  renderHomeLocal();
}

async function loadLocal() {
  try { localInfo = await invoke("local_info"); } catch { localInfo = null; }
  renderLocalPanel();
}

async function bench(id) {
  delete localErr[id];
  benching = true;
  renderLocalPanel();
  try { await invoke("local_bench", { id }); }
  catch (err) { localErr[id] = `测速失败：${err}`; }
  benching = false;
  loadLocal();
}
$("#hLocal").addEventListener("click", (e) => {
  const b = e.target.closest("[data-lbench]");
  if (b) bench(b.dataset.lbench);
});

listen("local-progress", ({ payload }) => {
  localProg[payload.model] = payload;
  const card = payload.model === "__move" ? document.querySelector(".lm-dir") : document.querySelector(`.lm[data-m="${payload.model}"]`);
  const bar = card?.querySelector(".lm-bar i"), txt = card?.querySelector(".lm-pct");
  if (bar && txt) {
    bar.style.width = `${payload.total ? (payload.done / payload.total) * 100 : 0}%`;
    txt.textContent = progressText(payload);
  } else {
    renderLocalPanel();
  }
});
listen("local-changed", loadLocal);

$("#plist").addEventListener("click", async (e) => {
  const t = e.target.closest("button");
  if (!t) return;
  if (t.hasAttribute("data-ldir")) {
    const dir = await invoke("local_pick_dir");
    if (dir) relocate(dir);
    return;
  }
  if (t.hasAttribute("data-ldirreset")) { relocate(null); return; }
  if (t.dataset.ldl) {
    const id = t.dataset.ldl;
    delete localErr[id];
    delete localProg[id];
    localInfo.downloading = id;
    renderLocalPanel();
    try { await invoke("local_download", { id }); }
    catch (err) { if (String(err) !== "已取消") localErr[id] = `下载没有完成：${err}。再点一次「下载」会从断点继续。`; }
    loadLocal();
  } else if (t.dataset.lbench) {
    bench(t.dataset.lbench);
  } else if (t.hasAttribute("data-lcancel")) {
    invoke("local_cancel");
  } else if (t.dataset.luse) {
    settings.providers.local = { ...settings.providers.local, model: t.dataset.luse };
    saveSoon();
    renderLocalPanel();
    renderList(false);
  } else if (t.dataset.ldel) {
    const id = t.dataset.ldel;
    // Deleting a multi-gigabyte download asks for a second click.
    if (confirmDelete !== id) { confirmDelete = id; renderLocalPanel(); setTimeout(() => { if (confirmDelete === id) { confirmDelete = null; renderLocalPanel(); } }, 4000); return; }
    confirmDelete = null;
    try {
      await invoke("local_delete", { id });
      if (settings.providers.local?.model === id) { settings.providers.local.model = ""; saveSoon(); renderList(false); }
    } catch (err) { localErr[id] = String(err); }
    loadLocal();
  }
});
loadLocal();
$("#tFallback").onclick = () => { settings.auto_fallback = !settings.auto_fallback; toggle($("#tFallback"), settings.auto_fallback); saveSoon(); };
$("#tContext").onclick = () => { settings.context_lines = settings.context_lines > 0 ? 0 : 2; toggle($("#tContext"), settings.context_lines > 0); saveSoon(); };

// ── subtitle style (live preview here, applied to the overlay by the backend) ──
function applyStylePreview(s) {
  root.style.setProperty("--sub", `${s.size}px`);
  root.style.setProperty("--plate", s.plate);
  const p = $("#pvw");
  p.classList.toggle("serif", s.font === "serif");
  p.classList.toggle("left", s.align === "left");
  p.classList.toggle("one", s.lines === 1);
  p.classList.toggle("zh-only", !s.show_en);
  const sz = $("#szR"); sz.value = s.size; fillRange(sz);
  const pl = $("#plR"); pl.value = Math.round(s.plate * 100); fillRange(pl);
  setSeg("#fontSeg", s.font); setSeg("#alignSeg", s.align); setSeg("#lineSeg", s.lines);
  toggle($("#enT"), s.show_en);
}
let styleTimer;
function updateStyle(patch) {
  settings.style = { ...settings.style, ...patch };
  applyStylePreview(settings.style);
  clearTimeout(styleTimer);
  styleTimer = setTimeout(() => invoke("update_style", { style: settings.style }), 60);
}
$("#szR").oninput = (e) => updateStyle({ size: +e.target.value });
$("#plR").oninput = (e) => updateStyle({ plate: e.target.value / 100 });
$("#enT").onclick = () => updateStyle({ show_en: !settings.style.show_en });
seg("#fontSeg", (b) => updateStyle({ font: b.dataset.v }));
seg("#alignSeg", (b) => updateStyle({ align: b.dataset.v }));
seg("#lineSeg", (b) => updateStyle({ lines: +b.dataset.v }));
listen("style-changed", ({ payload }) => { if (settings) { settings.style = payload; applyStylePreview(payload); } });
let demoN = 0;
$("#demo").onclick = () => invoke("overlay_demo", { n: demoN++ });

// ── region ──
async function refreshPreview() {
  const cv = $("#shotCv"), empty = $("#shotEmpty");
  if (!settings?.region) { cv.hidden = true; empty.hidden = false; return; }
  try {
    const buf = new Uint8Array(await invoke("capture_preview"));
    const dv = new DataView(buf.buffer);
    const w = dv.getUint32(0, true), h = dv.getUint32(4, true);
    cv.width = w; cv.height = h;
    cv.getContext("2d").putImageData(new ImageData(new Uint8ClampedArray(buf.buffer, 8, w * h * 4), w, h), 0, 0);
    cv.hidden = false; empty.hidden = true;
  } catch (err) {
    cv.hidden = true; empty.hidden = false; empty.textContent = String(err);
  }
}
function renderRegion() {
  const r = settings.region;
  $("#regionText").textContent = r ? `${r.w} × ${r.h} · (${r.x}, ${r.y})` : "尚未框选";
  setSeg("#freqSeg", [800, 1200, 2000].includes(settings.interval_ms) ? settings.interval_ms : 1200);
  toggle($("#tSkip"), settings.skip_unchanged);
}
$("#reselect").onclick = () => invoke("select_region");
$("#refresh").onclick = refreshPreview;
seg("#freqSeg", (b) => { settings.interval_ms = +b.dataset.v; saveSoon(); });
$("#tSkip").onclick = () => { settings.skip_unchanged = !settings.skip_unchanged; toggle($("#tSkip"), settings.skip_unchanged); saveSoon(); };
setInterval(() => { if (page === "region" && !document.hidden) refreshPreview(); }, 10000);

// ── settings from the backend ──
function applySettings(s) {
  settings = s;
  settings.providers ??= {};
  renderChain();
  renderList(false);
  toggle($("#tFallback"), s.auto_fallback);
  toggle($("#tContext"), s.context_lines > 0);
  applyStylePreview(s.style);
  renderRegion();
  homeSub();
  // The first-run hint goes away once a region exists.
  const msg = $("#hMsg");
  if (s.region && msg.textContent.startsWith("第一步")) msg.hidden = true;
}
listen("settings-changed", ({ payload }) => {
  // Our own save echoing back: keep the page as it is, so an input being typed in keeps focus.
  if (settings && signature(payload) === lastSent) { settings.overlay = payload.overlay; settings.main_window = payload.main_window; return; }
  const wasRegion = JSON.stringify(settings?.region);
  applySettings(payload);
  if (JSON.stringify(payload.region) !== wasRegion && page === "region") refreshPreview();
});

(async () => {
  applySettings(await invoke("get_settings"));
  const snap = await invoke("get_state");
  setState(snap.state);
  toggle($("#ovLock"), snap.locked);
  if (snap.ocr) $("#ocrText").textContent = `Windows 文字识别 · ${snap.ocr}`;
  requestAnimationFrame(() => { placeInd(); $$(".seg").forEach(placeThumb); });
  document.fonts?.ready.then(() => { placeInd(); $$(".seg").forEach(placeThumb); });
  if (!settings.region) showMsg("warn", "第一步：到「识别区域」框选 MultiViewer 的无线电转录卡片。");
})();
addEventListener("resize", () => { placeInd(); $$(".seg").forEach(placeThumb); });
