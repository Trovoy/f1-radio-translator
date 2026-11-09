**English** | [简体中文](README.zh-CN.md)

<div align="center">
  <img src="app-icon.png" width="112" alt="F1 Radio headset icon" />
  <h1>F1 Radio Translator</h1>
  <p>Live Chinese subtitles for F1 team radio, laid over the race.</p>
  <p>Windows 10/11 · Tauri + Rust · Windows OCR · 24 translation services · Local GPU models · MIT</p>
</div>

A Windows companion for **MultiViewer AI Radio Transcriptions**. Select the transcript area once; new radio lines are read with the OCR built into Windows, translated into Simplified Chinese and shown in a transparent subtitle window that sits on top of the race video.

Version 2.0 is a full rewrite in Tauri 2 and Rust. It no longer needs Python, and the installer is under 3 MB. The interface is in Chinese.

[Download the latest release](https://github.com/Trovoy/f1-radio-translator/releases/latest) · [Build guide](docs/build.md) · [Privacy](docs/privacy.md) · [Changelog](CHANGELOG.md)

## Features

- **Subtitle window over the video:** transparent and always on top. At rest it shows only text; hovering reveals a glass toolbar. Lock it to make it click-through. Position and size are remembered.
- **Driver name plates:** speaker names stay in English with a team-colour bar, so it is always clear who is talking. OCR slips are snapped to the current grid.
- **Region selection on any monitor:** draw the transcript area on any screen, with a docked size label and confirm/cancel tools. Double-click or Enter confirms, Esc cancels.
- **24 translation services with fallback:** DeepL, Google, Microsoft, Baidu, Youdao, Tencent, Alibaba, Volcano, MyMemory, OpenAI, Claude, Gemini, DeepSeek, Qwen, GLM, Kimi, Doubao, OpenRouter, SiliconFlow, any OpenAI-compatible endpoint, Ollama, LM Studio, LibreTranslate and DeepLX. Order them as primary and backups; a service answering 429 sits out for a minute.
- **Built-in local GPU translation:** pick a model and the app downloads llama.cpp and the model and runs it on your graphics card, fully offline. Choose from Qwen3 8B (recommended), Qwen3 14B, HY-MT1.5 7B and HY-MT1.5 1.8B. The panel shows generation speed, latency, time to first token, VRAM and GPU load, and has a one-click speed test.
- **F1-aware translation:** a radio glossary (box, set eight, lift and coast, brake bias, boost…), driver-name protection, slang rewriting for machine translation, and per-driver context for language models.
- **Global hotkeys and tray menu:** control everything without leaving the race.

## Quick start

1. Download and run `F1 Radio_2.0.0_x64-setup.exe` from [Releases](https://github.com/Trovoy/f1-radio-translator/releases).
2. In MultiViewer, open **AI Radio Transcriptions**.
3. In the app, open **识别区域** (capture region) and draw a box around the transcript cards.
4. Under **翻译服务** (translation services), configure a service. MyMemory works without a key. To translate offline, open **本地与自建 → 本地模型（显卡）** (local and self-hosted → local model), download a model, click **使用此模型** (use this model), then **设为主用** (set as primary).
5. Click **开始翻译** (start translating) on the overview page.

Windows OCR needs the English (United States) language. If the app reports a missing OCR language, add it under Settings → Time & language → Language.

## Hotkeys

| Keys | Action |
| --- | --- |
| Ctrl + Alt + F9 | Start / pause |
| Ctrl + Alt + F10 | Stop |
| Ctrl + Alt + L | Lock / unlock the subtitle window (click-through) |
| Ctrl + Alt + H | Show / hide the subtitle window |
| Ctrl + Alt + , | Open the main window |

## Local models

| Model | Download | VRAM | Notes |
| --- | --- | --- | --- |
| Qwen3 8B (recommended) | 5.0 GB | ~6.5 GB | Understands radio context, follows the glossary and context. About 0.26 s per line and 70 tok/s on an RTX 4070 Ti. |
| Qwen3 14B | 9.0 GB | ~10 GB | Stronger, for 12 GB cards and up. |
| HY-MT1.5 7B | 4.6 GB | ~6 GB | Tencent's translation model; literal on fragmented radio lines. |
| HY-MT1.5 1.8B | 1.1 GB | ~2 GB | Very fast, lower quality; for weak GPUs. |

On NVIDIA cards, the engine is llama.cpp's CUDA build (about 680 MB). Other GPUs get the Vulkan build (about 38 MB). The engine and models are downloaded from GitHub and ModelScope (with Hugging Face as a fallback) into `%LOCALAPPDATA%\com.trovoy.f1radio\local`. Downloads resume after an interruption. Models can be deleted in the app.

## Build from source

Requires Node.js 20+, Rust (rustup) and the Visual Studio C++ build tools.

```powershell
git clone https://github.com/Trovoy/f1-radio-translator.git
cd f1-radio-translator
npm install
npx tauri dev      # development
npx tauri build    # installer in src-tauri/target/release/bundle/nsis/
```

See the [build guide](docs/build.md) for tests, debug switches and the project layout.

## Privacy and limitations

Screenshots and OCR stay on your computer. Only the recognized transcript text (and, for language models, a few earlier lines from the same driver) goes to the translation service you choose. With a local model, nothing leaves the computer. API keys are encrypted with Windows DPAPI. Read [Privacy](docs/privacy.md) for details.

The app reads on-screen transcripts, not race audio, and does not modify MultiViewer. Transcript quality, OCR, network and provider limits affect accuracy and delay.

## License

Original source is [MIT licensed](LICENSE). Third-party components and downloaded models keep their own terms. See [third-party notices](THIRD_PARTY_NOTICES.md). This is an independent hobby project, unaffiliated with Formula 1, MultiViewer or the translation providers.
