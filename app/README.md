# F1 Radio 2.0（Tauri + Rust）

新版用 Tauri 2 重写：透明字幕窗压在比赛画面上，主窗口负责开始、选服务和调样式。文字识别改用 Windows 自带的 `Windows.Media.Ocr`，不再需要 Python。

## 开发

需要 Node.js、Rust（rustup）和 Visual Studio C++ 生成工具。

```powershell
cd app
npm install
npx tauri dev
```

调试版支持几个环境变量，正式版会忽略它们：

| 变量 | 作用 |
| --- | --- |
| `F1RADIO_DEMO=1` | 字幕窗自动播放几句示例字幕，便于调样式 |
| `F1RADIO_AUTOSTART=1` | 启动 3 秒后自动开始翻译 |
| `F1RADIO_SETTINGS=路径` | 改用指定的设置文件，测试时不碰真实设置 |
| `F1RADIO_DRYRUN=1` | 只打印会使用哪块显示器，然后退出，不显示任何窗口 |

显示器按左上角坐标记忆（`display_at`），名称只作备用：Windows 重启后可能把 DISPLAY1 和 DISPLAY2 对调。

## 打包

```powershell
npx tauri build
```

安装包输出在 `src-tauri/target/release/bundle/nsis/`。

## 测试

```powershell
cd src-tauri
cargo test --lib -- --include-ignored
```

`ocr_reads_multiviewer_cards` 用 `tests/card.png` 这张模拟转录卡片检查识别和说话人拆分；`mymemory_translates` 会联网调用 MyMemory。

## 结构

| 位置 | 作用 |
| --- | --- |
| `index.html` / `src/main.js` | 主窗口：概览、翻译服务、字幕样式、识别区域、快捷键 |
| `overlay.html` / `src/overlay.js` | 透明字幕窗，悬停显示工具栏，可锁定为鼠标穿透 |
| `selector.html` / `src/selector.js` | 全屏框选识别区域 |
| `src/providers.js` | 24 家翻译服务的字段定义 |
| `src-tauri/src/vision.rs` | GDI 截屏、Windows OCR、MultiViewer 卡片拆分 |
| `src-tauri/src/translate.rs` | 四类翻译适配器和 F1 术语表 |
| `src-tauri/src/engine.rs` | 识别线程、翻译并发、失败自动切换 |
| `src-tauri/src/settings.rs` | 设置读写，密钥用 Windows DPAPI 加密 |

设置保存在 `%APPDATA%\com.trovoy.f1radio\settings.json`。

## 快捷键（全局）

Ctrl Alt F9 开始/暂停 · Ctrl Alt F10 结束 · Ctrl Alt L 锁定字幕窗 · Ctrl Alt H 显示/隐藏字幕窗 · Ctrl Alt , 打开主窗口
