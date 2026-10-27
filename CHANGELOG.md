**English** | [简体中文](CHANGELOG.zh-CN.md)

# Changelog

## 1.0.2 — Open-source release

- Retained screen OCR, English–Chinese output, speaker labels, F1 context and multiple translation providers.
- Retained capture handles, enlarged preview, ten-second thumbnail refresh, compact mode and tray controls.
- Used UTF-8 stdin bytes on Windows PowerShell 5.1 instead of the unavailable `StandardInputEncoding` property.
- Managed launcher-owned worker shutdown with a Windows Job Object.
- Removed personal development paths and added portable build scripts, the MIT license and product documentation.
- Fixed Python-version detection argument quoting in the build script on Windows PowerShell 5.1.
- Published a Windows x64 EXE, portable ZIP and SHA-256 checksums.
- Excluded local settings, credentials, screenshots, dependencies and generated binaries from source control.
