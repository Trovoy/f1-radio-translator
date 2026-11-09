**English** | [简体中文](build.zh-CN.md)

# Build and run

## Requirements

- 64-bit Windows 10/11 with WebView2 (built into current Windows).
- [Node.js](https://nodejs.org/) 20 or newer.
- [Rust](https://rustup.rs/) (stable, MSVC toolchain) and the Visual Studio C++ build tools.

No Python, personal development path or author account is needed.

## Develop

```powershell
npm install
npx tauri dev
```

Debug builds read a few environment variables; release builds ignore them:

| Variable | Effect |
| --- | --- |
| `F1RADIO_DEMO=1` | The subtitle window plays sample lines, handy for styling |
| `F1RADIO_AUTOSTART=1` | Starts translating three seconds after launch |
| `F1RADIO_SETTINGS=<path>` | Uses another settings file, so tests never touch your real settings |
| `F1RADIO_DRYRUN=1` | Prints where each window would open, then exits without showing any |

## Package

```powershell
npx tauri build
```

The NSIS installer is written to `src-tauri/target/release/bundle/nsis/`. The llama.cpp engine and local models are not bundled; the app downloads them on demand.

## Test

```powershell
cd src-tauri
cargo test --lib
```

The default tests run offline: OCR on the sample cards in `tests/`, card splitting, speaker snapping, name protection, slang rewriting and prompt building. Ignored tests need extra resources:

| Test | Needs |
| --- | --- |
| `mymemory_translates`, `mymemory_keeps_driver_names` | Network |
| `deepl_samples_from_user_settings` | Network and a DeepL key in your own settings |
| `ocr_dump_file` | `F1RADIO_OCR_FILE` pointing to a raw capture |
| `local_model_end_to_end` | Downloads the engine and a model (gigabytes) and uses the GPU; pick the model with `F1RADIO_LOCAL_MODEL` |

Run one with `cargo test --lib <name> -- --ignored --nocapture`.

## Layout

| Path | Purpose |
| --- | --- |
| `index.html`, `src/main.js` | Main window: overview, translation services, local models, subtitle style, capture region, hotkeys |
| `overlay.html`, `src/overlay.js` | Transparent subtitle window with hover toolbar and click-through lock |
| `selector.html`, `src/selector.js` | Region selection, one window per monitor |
| `traymenu.html`, `src/traymenu.js` | Tray menu |
| `src/providers.js` | Field definitions for the translation services |
| `src-tauri/src/vision.rs` | GDI capture, Windows OCR, MultiViewer card parsing, driver grid |
| `src-tauri/src/translate.rs` | Translation adapters, F1 glossary, prompts |
| `src-tauri/src/local.rs` | llama.cpp engine and model downloads, server lifecycle, speed stats |
| `src-tauri/src/engine.rs` | Capture loop, concurrent translation, fallback chain |
| `src-tauri/src/settings.rs` | Settings file; secrets encrypted with Windows DPAPI |
| `src-tauri/src/lib.rs` | Windows, tray, hotkeys and commands |

## Local data

| Data | Location |
| --- | --- |
| Settings | `%APPDATA%\com.trovoy.f1radio\settings.json` |
| Engine and models | `%LOCALAPPDATA%\com.trovoy.f1radio\local` (changeable in the app) |

[Back to README](../README.md)
