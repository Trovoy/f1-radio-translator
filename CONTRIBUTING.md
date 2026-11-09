**English** | [简体中文](CONTRIBUTING.zh-CN.md)

# Contributing

Improvements to OCR, F1 terminology, translation prompts, the interface and builds are welcome.

1. Follow the [build guide](docs/build.md) to install Node.js and Rust, then run `npm install`.
2. Make changes on a branch and run `cargo test --lib` in `src-tauri`. Explain the problem, the resulting behaviour and how you checked it.
3. Open a pull request; include redacted screenshots for interface changes when useful.

Do not commit API keys, account settings, private paths, real race screenshots with driver photos, local logs, models or binaries. Update the third-party notices when adding a dependency.

Distinguish original transcript errors, OCR mistakes and translation errors. Reports should include reproduction steps, the Windows version, the translation service or local model and redacted sample text, without keys or credential-bearing URLs. Do not claim a full race-session check when only builds or unit tests were run.
