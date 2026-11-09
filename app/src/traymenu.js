import { getCurrentWindow } from "@tauri-apps/api/window";
import { $, invoke, listen, mark } from "./shared.js";

const win = getCurrentWindow();
const PLAY = '<path d="M4.5 2.8v10.4L13 8z"/>';
const PAUSE = '<rect x="4" y="3" width="2.8" height="10" rx=".7"/><rect x="9.2" y="3" width="2.8" height="10" rx=".7"/>';
let snap = { state: "idle", locked: false, overlay_visible: true };

$("#mk").innerHTML = mark();

function render() {
  const s = snap.state;
  $("#dot").className = "dot" + (s === "live" ? " live" : s === "paused" ? " paused" : "");
  $("#stText").textContent = { idle: "未开始", live: "翻译中", paused: "已暂停" }[s];
  $("#playIc").innerHTML = s === "live" ? PAUSE : PLAY;
  $("#toggleText").textContent = s === "live" ? "暂停" : s === "paused" ? "继续" : "开始翻译";
  $("#stopBtn").disabled = s === "idle";
  $("#lockText").textContent = snap.locked ? "解锁字幕窗" : "锁定字幕窗";
  $("#ovText").textContent = snap.overlay_visible ? "隐藏字幕窗" : "显示字幕窗";
}

async function refresh() {
  snap = await invoke("get_state");
  render();
  const m = $("#menu");
  m.classList.remove("in"); void m.offsetWidth; m.classList.add("in");
}

const close = () => win.hide();
$("#menu").addEventListener("click", async (e) => {
  const b = e.target.closest("button");
  if (!b) return;
  close();
  const act = b.dataset.act;
  if (act === "toggle") invoke("engine_cmd", { action: "toggle" });
  else if (act === "stop") invoke("engine_cmd", { action: "stop" });
  else if (act === "lock") invoke("set_overlay_locked", { locked: !snap.locked });
  else if (act === "overlay") invoke("set_overlay_visible", { visible: !snap.overlay_visible });
  else if (act === "main") invoke("open_main", {});
  else if (act === "quit") invoke("quit");
});

// Like a native menu: it goes away when focus moves elsewhere or Esc is pressed.
window.addEventListener("blur", close);
document.addEventListener("keydown", (e) => { if (e.key === "Escape") close(); });
listen("traymenu-open", refresh);
refresh();
