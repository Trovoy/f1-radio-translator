**English** | [简体中文](privacy.zh-CN.md)

# Privacy and local data

## Data flow

```text
Selected screen region
  → local screenshot
  → local RapidOCR
  → recognized subtitle body queue
  → your selected translation endpoint
  → local English–Chinese window
```

- Screenshots and enlarged previews are used locally; translation requests do not upload images.
- New subtitle body text goes to the selected provider. Speaker labels extracted from the name line are not appended.
- OCR grouping/recognition can be wrong. Private text inside the selected region may also be recognized and transmitted as subtitle text.
- GPT/DeepL requests use your credentials. The app supplies no shared key or author-hosted proxy.

## Stored settings

The settings file is `%LOCALAPPDATA%\MultiViewerRadioTranslator\settings.json`.

Keys are runtime-only by default. Enabling “记住密钥” (remember key) stores them encrypted with the current Windows user's DPAPI. Capture bounds, provider and Base URL are ordinary settings; the URL is not automatically encrypted, so do not embed secrets in it. Disabling key remembrance removes the corresponding saved key.

Temporary OCR screenshots are stored in the system temporary directory and removed on normal exit; crashes or power loss can leave files behind. The EXE dependency cache is separate from user settings and is not a translation-history database.

## Repository contents

The repository contains public source, icons, build scripts and documentation, not user settings, real keys, DPAPI ciphertext, screenshots, chats, logs, login credentials or private development paths. Dependencies, runtimes and generated binaries are excluded from Git.

## Reporting problems

Redact API keys, authorization headers, account details, private paths and secret-bearing provider URLs. Never attach `settings.json`, `.env` or authorization files to an issue.

[Back to README](../README.md)
