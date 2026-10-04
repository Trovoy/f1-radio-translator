<div align="center">
  <img src="src/Translator-logo.png" width="112" alt="F1 Radio Translator 赛车与无线电图标" />
  <h1>F1 Radio Translator</h1>
  <p>跟上赛道节奏，读懂车队无线电。</p>
  <p>Windows 桌面工具 · 本地 OCR · 英中双语 · MIT 开源</p>
</div>

F1 Radio Translator 是为 MultiViewer 无线电转录窗口打造的独立中文翻译小窗。框选屏幕上的英文转录后，软件在本机识别新句子，并调用你选择的翻译接口显示简体中文。英文原文和说话人标签同时保留，便于边看比赛边核对语境。

[产品详情](docs/product-page.md) · [可直接打开的详情页](docs/index.html) · [构建说明](docs/build.md) · [隐私说明](docs/privacy.md) · [版本记录](CHANGELOG.md)

## 功能

- **字幕区域框选**：蓝色选框、八个拖拽手柄、坐标与尺寸标签；支持移动、缩放、方向键微调、确认和取消。
- **识别区域预览**：缩略图每 10 秒更新，可放大查看、缩放和滚动浏览；选区自动保存，也可清除。
- **英中双语显示**：保留英文原文，中文译文更醒目；识别到的说话人姓名作为本地标签显示。
- **多种翻译接口**：MyMemory、DeepL，以及使用 Chat Completions 的 OpenAI 兼容接口。当前 GPT 选项的模型名为 `gpt-6-luna`，上游须支持该名称或对应别名。
- **F1 语境提示**：为术语、旗语、进站和能量部署等内容提供语境，并要求保留不确定的识别内容。
- **观赛小窗**：深色 WPF 界面、手动简洁模式和细滚动条。
- **托盘控制**：收起后继续运行，支持恢复、暂停/继续、结束翻译和退出；点 X 退出软件。
- **本机记忆设置**：选区、服务和上游地址保存在本机；密钥可选用 Windows 当前用户 DPAPI 加密保存，并支持显示/隐藏。
- **减少重复处理**：常驻 OCR、画面变化检测、文本去重、请求队列和退避；显示最多保留最近 120 条。

## 快速开始

### 源码运行

需要 64 位 Windows 10/11、Windows PowerShell 5.1 和 64 位 Python 3.12。

```powershell
git clone https://github.com/Trovoy/f1-radio-translator.git
cd f1-radio-translator
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Setup-Dependencies.ps1
.\src\Start-MultiViewerTranslator.bat
```

打开 MultiViewer 的 AI Radio Transcriptions，点击“框选字幕区域”，把卡片正文与下方姓名/时间行一起框入。确认区域后，选择翻译服务并开始翻译。

### EXE 构建

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build-Executable.ps1
```

输出在 `dist/`。完整 EXE 包含 Python 和 OCR 依赖，首次运行会在本机应用数据目录展开，之后复用缓存。预编译版本见 [GitHub Releases](https://github.com/Trovoy/f1-radio-translator/releases)：可直接下载 EXE，也可下载包含使用说明与许可证的便携 ZIP。详细参数见[构建说明](docs/build.md)。

## 翻译服务配置

| 选项 | 需要填写 | 说明 |
| --- | --- | --- |
| MyMemory | 无密钥 | 备用接口，额度与可用性由服务决定 |
| DeepL API | 用户自己的 API Key | 使用 DeepL API；账户额度与计划由服务决定 |
| GPT-6 Luna | API Key、可选 Base URL | 上游须提供 `/chat/completions` 和 `gpt-6-luna` 模型/别名；Base URL 留空使用应用的默认地址 |

例如，上游给出的地址为 `https://api.example.com/v1`，应用会请求其 `/chat/completions`。不要把账户登录凭证或后台管理密钥当作推理接口密钥。第三方接口的费用、模型权限和速率限制由你使用的服务决定。

## 快捷键与托盘

主窗口有焦点时：F8 切换简洁模式，F9 暂停/继续，F10 结束翻译。框选时：Enter 确认、Esc 取消、R 重选，方向键移动 1 像素，Shift + 方向键移动 10 像素。

点击“收起到托盘”或最小化可隐藏窗口；双击托盘图标恢复。点 X 或托盘“退出软件”会清理工作进程并退出。EXE 启动器另外使用 Windows Job Object 管理其所属进程树。

## 隐私与边界

截图与 OCR 在本机完成，应用不会将截图作为翻译请求上传。识别后的字幕正文会发给所选翻译服务；说话人标签不附加到翻译请求。密钥、选区和上游地址位于 `%LOCALAPPDATA%\MultiViewerRadioTranslator\settings.json`，不属于开源发布文件。

本项目读取的是屏幕上的转录文字，不直接获取或识别比赛音频，也不修改 MultiViewer。OCR 与翻译结果会受到字幕尺寸、画面清晰度、原始转录质量、网络和服务限制影响；“实时”是轮询更新，不能保证零延迟或术语完全准确。更多见[隐私说明](docs/privacy.md)。

## 开发与许可

- `src/`：WPF 主程序、本地 OCR 工作进程、区域选择器和图标。
- `build/`：EXE 启动器与 Windows 清单。
- `scripts/`：依赖准备、通用 Windows 构建流程。
- `docs/`：产品详情页、构建与隐私说明。

欢迎通过 [Issues](https://github.com/Trovoy/f1-radio-translator/issues) 反馈问题。提交错误截图或日志前，请遮住密钥、用户名、私人路径和上游敏感信息。

原创源码使用 [MIT](LICENSE)，第三方组件见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。这是独立的兴趣项目，与 F1、MultiViewer 及翻译服务提供方无官方关联。
