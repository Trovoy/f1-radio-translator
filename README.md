**English** | [简体中文](README.zh-CN.md)

<div align="center">
  <img src="src/Translator-logo.png" width="112" alt="F1 Radio Translator racing and radio icon" />
  <h1>F1 Radio Translator</h1>
  <p>Read team radio without losing track of the race.</p>
  <p>Windows desktop · Local OCR · English–Chinese subtitles · MIT</p>
</div>

A Windows companion for **MultiViewer AI Radio Transcriptions**. Select the English transcript region on screen, recognize new lines locally and display Simplified Chinese translations from your chosen service. English originals and detected speaker labels stay visible for context. The application interface is currently Chinese; English documentation does not imply an English UI.

[Download the latest release](https://github.com/Trovoy/f1-radio-translator/releases/latest) · [Product overview](docs/product-page.md) · [Build guide](docs/build.md) · [Privacy](docs/privacy.md) · [Changelog](CHANGELOG.md)

## Features

- **Adjustable capture region:** move/resize the blue selection with eight handles, coordinate/size labels and keyboard nudges.
- **Capture preview:** a thumbnail refreshes every ten seconds; open it to zoom and scroll. The region is saved locally and can be cleared.
- **Bilingual output:** larger Chinese translations below the English source, with locally detected speaker labels.
- **Translation services:** MyMemory, DeepL API and an OpenAI-compatible Chat Completions endpoint. The current GPT option requests `gpt-6-luna`; your provider must support that name or an alias.
- **F1 context:** prompts cover flags, pit stops, track limits and energy deployment while asking the model to retain uncertain transcript fragments.
- **Viewing window:** dark WPF interface, manual compact mode and tray controls for restore, pause/resume, stop and exit.
- **Local preferences:** save the region/provider/Base URL; optionally encrypt remembered keys with the current Windows user's DPAPI.
- **Less duplicate work:** persistent OCR worker, frame-change checks, text deduplication, a request queue and backoff; up to 120 displayed entries.

## Quick start

### Use a release

On **64-bit Windows 10/11**, download the EXE or portable ZIP from [Releases](https://github.com/Trovoy/f1-radio-translator/releases). Both contain the same application. The full EXE includes Python/OCR dependencies and extracts them to a local application-data cache on first launch. Translation credentials are configured separately by you.

1. Open MultiViewer's AI Radio Transcriptions.
2. In the translator, choose **“框选字幕区域”** (select subtitle region). Include the card body and the speaker/time line below it, then confirm the preview.
3. Select/configure a translation service and choose **“开始翻译”** (start translation).

### Run from source

Requires Windows PowerShell 5.1 and a **full 64-bit Python 3.12 installation** in addition to Windows 10/11.

```powershell
git clone https://github.com/Trovoy/f1-radio-translator.git
cd f1-radio-translator
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Setup-Dependencies.ps1
.\src\Start-MultiViewerTranslator.bat
```

To build the EXE:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build-Executable.ps1
```

Outputs go to `dist/`. See the [build guide](docs/build.md) for Python selection, runtime files and cache locations.

## Configure translation

| Option | Required settings | Notes |
| --- | --- | --- |
| MyMemory | No API key | Fallback service; provider quotas/availability apply. |
| DeepL API | Your API key | Your DeepL account plan and quota apply. |
| GPT-6 Luna | API key, optional Base URL | Provider must implement `/chat/completions` and the `gpt-6-luna` model/name alias. |

For a Base URL such as `https://api.example.com/v1`, the application calls its `/chat/completions` endpoint. The default source-code endpoint is `https://api.openai.com/v1`; model access must be available on the provider you actually use. Do not enter account passwords or dashboard admin keys as inference API keys. Fees, model access and rate limits belong to the selected service.

## Keyboard and tray controls

| Context | Keys / action |
| --- | --- |
| Main window focused | F8: compact mode · F9: pause/resume · F10: stop translation |
| Region selection | Enter: confirm · Esc: cancel · R: reselect |
| Region selection | Arrow: move 1 px · Shift + arrow: move 10 px |
| Tray | Double-click: restore; menu: pause/resume, stop, exit |

Minimize or use the tray button to hide the window while keeping work running. The window's **X exits** the application and cleans up workers. The EXE launcher additionally manages its process tree using a Windows Job Object.

## Privacy and limitations

Screenshots and OCR stay on the computer; images are not uploaded as translation requests. Recognized subtitle **body text** goes to the selected translation provider; speaker labels are not added. Settings live at `%LOCALAPPDATA%\MultiViewerRadioTranslator\settings.json` and are not part of the source/release files. Read [Privacy](docs/privacy.md) before capturing sensitive content or sharing diagnostics.

This reads on-screen transcripts, not race audio, and does not modify MultiViewer. Transcript quality, font size, OCR, network and provider limits affect accuracy and delay. Updates are polled, with no guarantee of zero latency or perfect terminology.

## Development and license

| Directory | Purpose |
| --- | --- |
| `src/` | WPF app, local OCR worker, region selector and icons |
| `build/` | EXE launcher and Windows manifest |
| `scripts/` | Dependency setup and portable Windows build scripts |
| `docs/` | Product, build, privacy and release documentation |

See [Contributing](CONTRIBUTING.md) before opening a PR. Report reproducible problems in [Issues](https://github.com/Trovoy/f1-radio-translator/issues), with keys, account details, private paths and provider secrets redacted.

Original source is [MIT licensed](LICENSE); bundled components retain their own [third-party terms](THIRD_PARTY_NOTICES.md). This is an independent hobby project, unaffiliated with F1, MultiViewer or the translation providers.
