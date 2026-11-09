**English** | [简体中文](privacy.zh-CN.md)

# Privacy and local data

## Data flow

```text
Selected screen region
  → screenshot in memory
  → Windows OCR (local)
  → new transcript lines
  → your chosen translation service, or the local model on your GPU
  → subtitle window
```

- Screenshots are kept in memory only and are never uploaded or written to disk.
- Cloud services receive the recognized line. DeepL and language models also receive a few earlier lines from the same driver as context, and language models receive the speaker's name.
- With the built-in local model, translation runs entirely on your computer and no transcript text leaves it.
- OCR can be wrong. Anything private inside the selected region may be recognized and sent as transcript text.
- Requests use your own credentials. The app has no shared key, author-hosted proxy or telemetry.

## Network access

Besides the translation service you configure, the app only connects to GitHub (the llama.cpp engine), ModelScope and Hugging Face (model files), and only when you download a local model.

## Stored data

| Data | Location |
| --- | --- |
| Settings: region, service order, style, window positions | `%APPDATA%\com.trovoy.f1radio\settings.json` |
| API keys and secrets | Same file, encrypted with the current Windows user's DPAPI |
| Local engine, models, engine log | `%LOCALAPPDATA%\com.trovoy.f1radio\local` |

Base URLs and model names are stored as plain settings, so do not embed secrets in a URL. Uninstalling the app leaves these folders; delete them to remove all data.

## Repository contents

The repository contains source, icons, two sample transcript captures for the OCR tests (the driver photo removed) and documentation. It contains no user settings, real keys, DPAPI ciphertext, logs or private paths. Dependencies, models and binaries are excluded from Git.

## Reporting problems

Redact API keys, authorization headers, account details, private paths and secret-bearing URLs. Never attach `settings.json` or `.env` files to an issue.

[Back to README](../README.md)
