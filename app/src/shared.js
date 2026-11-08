import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export { invoke, listen };

export const $ = (s, root = document) => root.querySelector(s);
export const $$ = (s, root = document) => [...root.querySelectorAll(s)];

/** The team-headset mark: four strokes and a red "on air" dot. */
const MARK = [
  "M11 27 V24 A13 13 0 0 1 37 24 V27",
  "M11 25 C8 25 7.5 27 7.5 30 C7.5 33 8 35 11 35 C12.5 35 13.5 34 13.5 32.5 V27.5 C13.5 26 12.5 25 11 25 Z",
  "M37 25 C40 25 40.5 27 40.5 30 C40.5 33 40 35 37 35 C35.5 35 34.5 34 34.5 32.5 V27.5 C34.5 26 35.5 25 37 25 Z",
  "M37 35 C37 39 33.5 41 29 41",
];

export function mark({ draw = false } = {}) {
  const paths = MARK.map((d, i) => `<path d="${d}" pathLength="1" style="--k:${i}"/>`).join("");
  return `<svg class="mk${draw ? " draw" : ""}" viewBox="0 0 48 48" aria-hidden="true">${paths}<path class="r d" d="M25.5 41 h.01" pathLength="1" style="--k:4"/></svg>`;
}

export const ICON = {
  play: '<svg viewBox="0 0 16 16" fill="currentColor"><path d="M4.5 2.8v10.4L13 8z"/></svg>',
  pause: '<svg viewBox="0 0 16 16" fill="currentColor"><rect x="4" y="3" width="2.8" height="10" rx=".7"/><rect x="9.2" y="3" width="2.8" height="10" rx=".7"/></svg>',
  stop: '<svg viewBox="0 0 16 16" fill="currentColor"><rect x="3.5" y="3.5" width="9" height="9" rx="1.5"/></svg>',
  check: '<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3.5 8.5l3 3 6-7"/></svg>',
};

export const speakerLabel = (s) => (s && s.trim() ? s.trim() : "无线电");

export function escapeHtml(s) {
  return String(s ?? "").replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
