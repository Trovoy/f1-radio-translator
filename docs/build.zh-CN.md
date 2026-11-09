[English](build.md) | **简体中文**

# 构建与运行

## 环境要求

- 64 位 Windows 10/11，带 WebView2（新版 Windows 自带）。
- [Node.js](https://nodejs.org/) 20 或更新版本。
- [Rust](https://rustup.rs/)（stable，MSVC 工具链）和 Visual Studio C++ 生成工具。

不需要 Python，也不依赖任何个人开发路径或作者账号。

## 开发

```powershell
npm install
npx tauri dev
```

调试版支持几个环境变量，正式版会忽略它们：

| 变量 | 作用 |
| --- | --- |
| `F1RADIO_DEMO=1` | 字幕窗自动播放几句示例字幕，便于调样式 |
| `F1RADIO_AUTOSTART=1` | 启动 3 秒后自动开始翻译 |
| `F1RADIO_SETTINGS=路径` | 改用指定的设置文件，测试时不碰真实设置 |
| `F1RADIO_DRYRUN=1` | 只打印各窗口会打开的位置，然后退出，不显示任何窗口 |

## 打包

```powershell
npx tauri build
```

NSIS 安装包输出在 `src-tauri/target/release/bundle/nsis/`。llama.cpp 引擎和本地模型不打进安装包，由软件按需下载。

## 测试

```powershell
cd src-tauri
cargo test --lib
```

默认测试全部离线：用 `tests/` 里的示例卡片检查文字识别、卡片拆分、车手名对齐、名字保护、口语改写和提示词。被忽略的测试需要额外条件：

| 测试 | 需要 |
| --- | --- |
| `mymemory_translates`、`mymemory_keeps_driver_names` | 联网 |
| `deepl_samples_from_user_settings` | 联网，且你自己的设置里有 DeepL 密钥 |
| `ocr_dump_file` | 用 `F1RADIO_OCR_FILE` 指定一份原始截图 |
| `local_model_end_to_end` | 会下载引擎和模型（数 GB）并占用显卡；用 `F1RADIO_LOCAL_MODEL` 选模型 |

单独运行：`cargo test --lib <测试名> -- --ignored --nocapture`。

## 目录结构

| 位置 | 作用 |
| --- | --- |
| `index.html`、`src/main.js` | 主窗口：概览、翻译服务、本地模型、字幕样式、识别区域、快捷键 |
| `overlay.html`、`src/overlay.js` | 透明字幕窗，悬停显示工具栏，可锁定为鼠标穿透 |
| `selector.html`、`src/selector.js` | 框选识别区域，每块显示器各开一个 |
| `traymenu.html`、`src/traymenu.js` | 托盘菜单 |
| `src/providers.js` | 各翻译服务的字段定义 |
| `src-tauri/src/vision.rs` | GDI 截屏、Windows OCR、MultiViewer 卡片拆分、车手名单 |
| `src-tauri/src/translate.rs` | 翻译适配器、F1 术语表、提示词 |
| `src-tauri/src/local.rs` | llama.cpp 引擎和模型下载、推理服务启停、速度统计 |
| `src-tauri/src/engine.rs` | 识别循环、并发翻译、失败自动切换 |
| `src-tauri/src/settings.rs` | 设置读写，密钥用 Windows DPAPI 加密 |
| `src-tauri/src/lib.rs` | 窗口、托盘、快捷键和命令 |

## 本机数据

| 数据 | 位置 |
| --- | --- |
| 设置 | `%APPDATA%\com.trovoy.f1radio\settings.json` |
| 引擎和模型 | `%LOCALAPPDATA%\com.trovoy.f1radio\local`（可在软件里更改） |

[返回 README](../README.zh-CN.md)
