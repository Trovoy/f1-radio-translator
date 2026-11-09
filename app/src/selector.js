import { $, invoke } from "./shared.js";

const sel = $("#sel");
const MIN_W = 50, MIN_H = 20;
let rect = null; // CSS pixels within this window
let drag = null;

function render() {
  if (!rect) { sel.hidden = true; document.body.classList.remove("has-sel"); $("#ok").disabled = true; return; }
  Object.assign(sel.style, { left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.w}px`, height: `${rect.h}px` });
  sel.hidden = false;
  document.body.classList.add("has-sel");
  const s = devicePixelRatio;
  $("#size").textContent = `${Math.round(rect.w * s)} × ${Math.round(rect.h * s)}`;
  $("#ok").disabled = rect.w < MIN_W || rect.h < MIN_H;
}

function clamp(r) {
  const W = innerWidth, H = innerHeight;
  r.w = Math.max(1, Math.min(r.w, W)); r.h = Math.max(1, Math.min(r.h, H));
  r.x = Math.max(0, Math.min(r.x, W - r.w)); r.y = Math.max(0, Math.min(r.y, H - r.h));
  return r;
}

document.addEventListener("pointerdown", (e) => {
  if (e.button !== 0 || e.target.closest(".bar")) return;
  const handle = e.target.dataset?.h;
  const start = { x: e.clientX, y: e.clientY, r: rect && { ...rect } };
  if (handle) drag = { mode: handle, start };
  else if (e.target.closest(".sel")) drag = { mode: "move", start };
  else { drag = { mode: "draw", start }; rect = { x: e.clientX, y: e.clientY, w: 0, h: 0 }; }
  document.setPointerCapture?.(e.pointerId);
});

document.addEventListener("pointermove", (e) => {
  if (!drag) return;
  const { start } = drag, dx = e.clientX - start.x, dy = e.clientY - start.y;
  if (drag.mode === "draw") {
    rect = { x: Math.min(start.x, e.clientX), y: Math.min(start.y, e.clientY), w: Math.abs(dx), h: Math.abs(dy) };
  } else if (drag.mode === "move") {
    rect = clamp({ ...start.r, x: start.r.x + dx, y: start.r.y + dy });
  } else {
    let { x, y, w, h } = start.r;
    if (drag.mode.includes("e")) w = Math.max(MIN_W, w + dx);
    if (drag.mode.includes("s")) h = Math.max(MIN_H, h + dy);
    if (drag.mode.includes("w")) { const nw = Math.max(MIN_W, w - dx); x += w - nw; w = nw; }
    if (drag.mode.includes("n")) { const nh = Math.max(MIN_H, h - dy); y += h - nh; h = nh; }
    rect = clamp({ x, y, w, h });
  }
  render();
});

document.addEventListener("pointerup", () => {
  if (drag?.mode === "draw" && rect && (rect.w < 4 || rect.h < 4)) rect = drag.start.r ?? null;
  drag = null;
  render();
});

const finish = () => { if (rect && rect.w >= MIN_W && rect.h >= MIN_H) invoke("finish_region", { rect }); };
const cancel = () => invoke("cancel_region");
$("#ok").onclick = finish;
$("#cancel").onclick = cancel;

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") return cancel();
  if (e.key === "Enter") return finish();
  if (!rect) return;
  const step = e.shiftKey ? 10 : 1;
  const move = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[e.key];
  if (move) { e.preventDefault(); rect = clamp({ ...rect, x: rect.x + move[0], y: rect.y + move[1] }); render(); }
});

invoke("selector_info").then(({ region }) => {
  if (region && region.w > 0) { rect = clamp({ ...region }); render(); }
});
// Tell the backend the page is painted; only then does it hide the other windows.
requestAnimationFrame(() => requestAnimationFrame(() => invoke("selector_ready")));
