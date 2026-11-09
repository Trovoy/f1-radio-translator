import { getCurrentWindow } from "@tauri-apps/api/window";
import { $, $$, ICON, escapeHtml, invoke, listen, mark, speakerLabel } from "./shared.js";
import { CATEGORIES, PROVIDER, PROVIDERS, fieldsFor, isConfigured, providerName } from "./providers.js";

const win = getCurrentWindow();
const root = document.documentElement;
const RM = matchMedia("(prefers-reduced-motion: reduce)").matches;

let settings = null;
let state = "idle";
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
    providerName(settings.chain[0]),
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
  li.innerHTML = `<div class="m">${escapeHtml(speakerLabel(l.speaker))}</div><div class="z">${escapeHtml(l.zh)}</div>`;
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

function renderChain(fresh) {
  $("#chain").innerHTML = settings.chain.map((id, i) =>
    `${i ? '<span class="arr">→</span>' : ""}<span class="c${id === fresh ? " in" : ""}">${escapeHtml(providerName(id))}${settings.chain.length > 1 ? `<button data-rm="${id}" title="移出顺序" aria-label="移出">✕</button>` : ""}</span>`
  ).join("");
  homeSub();
}
$("#chain").addEventListener("click", (e) => {
  const id = e.target.dataset.rm;
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
      + `<div class="det"><div><div class="in">${fields}<div class="acts">${actions}</div></div></div></div></div>`;
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
      const r = await invoke("test_provider", { id, fields: fieldsFor(settings, id) });
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
