**English** | [简体中文](release-1.0.2.zh-CN.md)

# F1 Radio Translator v1.0.2

First open-source Windows x64 binary release for Windows 10/11.

## Downloads

- `F1-Radio-Translator-1.0.2.exe`: standalone EXE with Python/local OCR dependencies.
- `F1-Radio-Translator-1.0.2-Windows-x64.zip`: the same EXE, instructions and licenses; extract and run.
- `SHA256SUMS.txt`: SHA-256 checksums for the release assets.

Both packages run the same application. First launch extracts the runtime under `%LOCALAPPDATA%\MultiViewerRadioTranslator\app`, then reuses the cache. Configure your own translation credentials.

## Features and fixes

- Local OCR, English–Chinese translations and speaker labels.
- Resizable capture region, enlarged preview and ten-second thumbnail refresh.
- MyMemory, DeepL and OpenAI-compatible APIs. GPT requests `gpt-6-luna`; the upstream must support that name/alias.
- Manual compact mode, key visibility and optional locally encrypted key storage.
- Pause, stop and tray controls; window X exits and cleans up workers.
- Windows PowerShell 5.1 compatibility: UTF-8 stdin bytes replace `StandardInputEncoding`; Python-version detection quoting is fixed.
- Built from redacted public source, with the MIT license and third-party notices.

## Use

1. Run the EXE and open MultiViewer AI Radio Transcriptions.
2. Select the transcript body and speaker line; verify the preview.
3. Configure a provider/key and choose “开始翻译” (start translation).

With the main window focused: F8 toggles compact mode, F9 pauses/resumes and F10 stops translation. Double-click the tray icon to restore the window.

Source redaction, PowerShell syntax and EXE compilation were checked before release; a complete race-viewing session was not validated. Transcripts, OCR and machine translation may contain errors. This project is unaffiliated with F1/MultiViewer.

[Source and documentation](https://github.com/Trovoy/f1-radio-translator) · [Issues](https://github.com/Trovoy/f1-radio-translator/issues)
