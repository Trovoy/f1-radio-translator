**English** | [简体中文](THIRD_PARTY_NOTICES.zh-CN.md)

# Third-party notices

Original project source is MIT licensed. Third-party components and models keep their own terms; the project's MIT license does not replace them.

## Built into the app

| Component | Use | License |
| --- | --- | --- |
| [Tauri](https://tauri.app/) and its plugins | Windows, tray, hotkeys, single instance | MIT / Apache-2.0 |
| [windows-rs](https://github.com/microsoft/windows-rs) | Windows OCR, GDI capture, DPAPI, job objects | MIT / Apache-2.0 |
| [reqwest](https://github.com/seanmonstar/reqwest), [tokio](https://tokio.rs/), [serde](https://serde.rs/), [regex](https://github.com/rust-lang/regex), [zip](https://github.com/zip-rs/zip2) and other crates in `Cargo.lock` | Networking, async, parsing, archives | MIT / Apache-2.0 and similar permissive terms |
| [Vite](https://vitejs.dev/) | Frontend build (not shipped) | MIT |
| Windows.Media.Ocr, WebView2 | OCR and web view | Supplied by Windows; not relicensed here |

## Downloaded on demand

These are not in the repository or the installer. The app downloads them only when you choose a local model.

| Component | Source | License |
| --- | --- | --- |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) `llama-server` | GitHub releases | MIT |
| NVIDIA CUDA runtime (CUDA build only) | llama.cpp GitHub releases | NVIDIA CUDA EULA |
| [Qwen3 8B / 14B GGUF](https://huggingface.co/Qwen/Qwen3-8B-GGUF) | ModelScope / Hugging Face | Apache-2.0 |
| [HY-MT1.5 7B / 1.8B GGUF](https://huggingface.co/tencent/HY-MT1.5-7B-GGUF) | ModelScope / Hugging Face | Tencent's model license; read it on the model page before use |

The headset icon was drawn for this project and does not use official F1 or MultiViewer marks. F1, Formula 1, MultiViewer, driver and team names belong to their respective holders. This independent tool is not affiliated with them.
