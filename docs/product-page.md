**English** | [简体中文](product-page.zh-CN.md)

# F1 Radio Translator

## Read team radio without losing track of the race

An English–Chinese translation window for Windows race viewing. F1 Radio Translator reads MultiViewer's on-screen radio transcripts, recognizes English locally and presents Simplified Chinese using your selected service. Keep the race in view while checking team instructions, driver feedback and strategy; the English source remains alongside the translation.

### Start with a capture region

Move and resize a blue frame around the transcript card. Eight handles and coordinate/size labels help identify the region. A thumbnail shows what is being captured; open it to zoom and scroll. The selection is saved locally for the next session.

### Compare the translation with its source

Larger Chinese text appears below the English original. Include the name line to show detected speakers above the content. Manual compact mode keeps only the speaker and bilingual text visible; return to the controls when settings need adjusting. The app UI is currently Chinese.

### Add F1 context

Supported translation prompts include flags, pit stops, track limits and energy deployment, while preserving unconfirmed transcript fragments. Choose MyMemory, DeepL or an OpenAI-compatible upstream. The current GPT option requests `gpt-6-luna`; the provider must support that model name or an alias.

### Control your viewing window

Pause, resume, stop and clear translations separately. Minimize or send the app to the tray to keep it working in the background. Double-click the tray icon to restore; use its menu for controls and exit. The window's X exits and cleans up workers.

### Keep OCR local

Screenshots/OCR run on your computer. Only recognized subtitle body text goes to the translation service. Use a key for the session or optionally remember it with Windows current-user encryption. You can reveal/hide the field to check what was entered. The original source is MIT licensed and its build process is documented.

### Three steps to start

1. Open MultiViewer AI Radio Transcriptions.
2. Select the card body and speaker line; verify the preview.
3. Configure a provider and choose “开始翻译” (start translation).

Requires 64-bit Windows 10/11. The full EXE bundles the OCR runtime; source runs follow the build guide. Provider credentials, fees, quotas and model access are your responsibility.

Polling, OCR, the original transcript and machine translation all affect delay/accuracy. This independent tool supplies no race content, MultiViewer account or audio recognition and has no official affiliation with F1 or MultiViewer.

[Source](https://github.com/Trovoy/f1-radio-translator) · [Build and run](build.md) · [Privacy](privacy.md)
