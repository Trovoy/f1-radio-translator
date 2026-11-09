**English** | [简体中文](CHANGELOG.zh-CN.md)

# Changelog

## 2.0.1

- Choose where local models are stored (for example on drive D); the engine and models already downloaded are moved there, with progress across drives.
- Shows the free space on that drive.
- The overview shows the local model's speed, VRAM and GPU load while it is the primary service.
- The subtitle window's toolbar has a hide button (show it again with Ctrl Alt H or from the main window).
- Reselecting the capture region starts from a clean screen; the old region stays until a new one is confirmed.
- Visual polish: deeper layered background, hairline window frame, monochrome switches, lighter surfaces with a lit top edge, editorial home numbers and an on-air dot while translating.
- The sidebar collapses to an icon rail (remembered between launches), and its icons were redrawn in the logo's line style with a dot that lights red on the current page.
- The service order is now a live topology (transcript → services → subtitle window): drag nodes to reorder, click one to open its settings; the primary route carries flowing light while translating, and each translated line lights the route it took with its latency.

## 2.0.0 — Full rewrite

- Rewritten in Tauri 2 and Rust; no Python. The installer is under 3 MB.
- New transparent subtitle window over the video: text only at rest, glass toolbar on hover, click-through lock, remembered position.
- New dark-glass main window with overview, translation services, subtitle style, capture region and hotkeys pages, plus a line-drawn headset logo.
- OCR switched to the Windows built-in `Windows.Media.Ocr`; MultiViewer cards are split by their name and time rows, the panel header is filtered out, and speaker names are snapped to the current grid.
- Driver name plates in English with team colours.
- Region selection on any monitor, with docked size label and confirm tools.
- 24 translation services with primary/backup order and automatic fallback; 429 responses cool a service down for a minute.
- Built-in local GPU translation: downloads llama.cpp (CUDA or Vulkan) and a model (Qwen3 8B/14B, HY-MT1.5 7B/1.8B) on demand, with resumable downloads and live speed stats.
- F1 glossary, driver-name protection, slang rewriting for machine translation and per-driver context for language models.
- Global hotkeys, custom tray menu, single instance, DPAPI-encrypted secrets.
- The 1.x PowerShell/WPF version remains available at tag `v1.0.2`.

## 1.0.2 — Open-source release

- Retained screen OCR, English–Chinese output, speaker labels, F1 context and multiple translation providers.
- Retained capture handles, enlarged preview, ten-second thumbnail refresh, compact mode and tray controls.
- Used UTF-8 stdin bytes on Windows PowerShell 5.1 instead of the unavailable `StandardInputEncoding` property.
- Managed launcher-owned worker shutdown with a Windows Job Object.
- Removed personal development paths and added portable build scripts, the MIT license and product documentation.
- Fixed Python-version detection argument quoting in the build script on Windows PowerShell 5.1.
- Published a Windows x64 EXE, portable ZIP and SHA-256 checksums.
- Excluded local settings, credentials, screenshots, dependencies and generated binaries from source control.
