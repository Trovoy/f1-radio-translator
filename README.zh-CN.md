[English](README.md) | **简体中文**

<div align="center">
  <img src="app-icon.png" width="112" alt="F1 Radio 耳机图标" />
  <h1>F1 Radio Translator</h1>
  <p>把 F1 车队无线电实时翻译成中文字幕，压在比赛画面上看。</p>
  <p>Windows 10/11 · Tauri + Rust · Windows OCR · 24 家翻译服务 · 本地显卡模型 · MIT</p>
</div>

配合 **MultiViewer AI Radio Transcriptions** 使用。框选一次转录区域后，软件用 Windows 自带的文字识别读取新出现的无线电，翻译成简体中文，显示在一个透明的字幕窗里，直接压在比赛视频上。

2.0 用 Tauri 2 和 Rust 完全重写，不再需要 Python，安装包不到 3 MB。

[下载最新版](https://github.com/Trovoy/f1-radio-translator/releases/latest) · [构建说明](docs/build.zh-CN.md) · [隐私](docs/privacy.zh-CN.md) · [更新日志](CHANGELOG.zh-CN.md)

## 功能

- **透明字幕窗**：始终置顶，平时只显示文字，鼠标悬停才出现玻璃工具栏；锁定后鼠标可以穿透，位置和大小会记住。
- **车手名牌**：说话人保持英文原名，配车队颜色条，一眼看出是谁在说话；识别错的名字会自动对齐到本赛季车手名单。
- **任意屏幕框选**：在哪块显示器上都能框选，尺寸标签和确认按钮贴在框边上；双击或回车确认，Esc 取消。
- **24 家翻译服务，自动切换**：DeepL、Google、微软、百度、有道、腾讯、阿里云、火山、MyMemory、OpenAI、Claude、Gemini、DeepSeek、通义千问、智谱、Kimi、豆包、OpenRouter、硅基流动、任意 OpenAI 兼容接口、Ollama、LM Studio、LibreTranslate、DeepLX。可以排主用和备用，某家返回 429 时自动跳过一分钟。
- **内置本地显卡翻译**：选一个模型，软件自动下载 llama.cpp 和模型，调用显卡离线翻译。可选 Qwen3 8B（推荐）、Qwen3 14B、HY-MT1.5 7B、HY-MT1.5 1.8B。面板实时显示生成速度、单句耗时、首字延迟、显存和 GPU 占用，还能一键测速。
- **懂 F1 的翻译**：无线电术语表（box、set eight、lift and coast、brake bias、boost……）、车手名保护、机器翻译前的口语改写，大模型还会参考同一车手的前文。
- **全局快捷键和托盘菜单**：看比赛时不用切窗口。

## 快速开始

1. 在 [Releases](https://github.com/Trovoy/f1-radio-translator/releases) 下载 `F1 Radio_2.0.0_x64-setup.exe` 并安装。
2. 在 MultiViewer 里打开 **AI Radio Transcriptions**。
3. 在软件里进入 **识别区域**，框住转录卡片。
4. 在 **翻译服务** 里配置一家服务（MyMemory 不需要密钥）。想离线翻译：进入 **本地与自建 → 本地模型（显卡）**，下载模型，点 **使用此模型**，再点 **设为主用**。
5. 回到概览，点 **开始翻译**。

Windows 文字识别需要英语（美国）语言。如果提示缺少识别语言，到 设置 → 时间和语言 → 语言 里添加。

## 快捷键

| 按键 | 作用 |
| --- | --- |
| Ctrl + Alt + F9 | 开始 / 暂停 |
| Ctrl + Alt + F10 | 结束 |
| Ctrl + Alt + L | 锁定 / 解锁字幕窗（鼠标穿透） |
| Ctrl + Alt + H | 显示 / 隐藏字幕窗 |
| Ctrl + Alt + , | 打开主窗口 |

## 本地模型

| 模型 | 下载大小 | 显存 | 说明 |
| --- | --- | --- | --- |
| Qwen3 8B（推荐） | 5.0 GB | 约 6.5 GB | 懂无线电语境，遵守术语表和上下文。RTX 4070 Ti 上每句约 0.26 秒，约 70 tok/s。 |
| Qwen3 14B | 9.0 GB | 约 10 GB | 更强，适合 12 GB 及以上显卡。 |
| HY-MT1.5 7B | 4.6 GB | 约 6 GB | 腾讯翻译模型，碎片化短句偏直译。 |
| HY-MT1.5 1.8B | 1.1 GB | 约 2 GB | 很快但质量一般，适合显卡较弱的电脑。 |

推理引擎是 llama.cpp：N 卡用 CUDA 版（约 680 MB），其他显卡用 Vulkan 版（约 38 MB）。引擎和模型从 GitHub 和魔搭社区下载（失败时改用 Hugging Face），保存在 `%LOCALAPPDATA%\com.trovoy.f1radio\local`，下载中断可以续传，不需要的模型可以在软件里删除。

## 从源码构建

需要 Node.js 20+、Rust（rustup）和 Visual Studio C++ 生成工具。

```powershell
git clone https://github.com/Trovoy/f1-radio-translator.git
cd f1-radio-translator
npm install
npx tauri dev      # 开发
npx tauri build    # 安装包输出在 src-tauri/target/release/bundle/nsis/
```

测试、调试开关和目录结构见 [构建说明](docs/build.zh-CN.md)。

## 隐私与限制

截图和文字识别都在本机完成。只有识别出的转录文字（大模型还会带上同一车手的几句前文）会发给你选的翻译服务；用本地模型时什么都不会离开电脑。API 密钥用 Windows DPAPI 加密保存。详见 [隐私](docs/privacy.zh-CN.md)。

软件读取的是屏幕上的转录文字，不是比赛音频，也不会修改 MultiViewer。转录质量、识别、网络和服务商限制都会影响准确度和延迟。

## 许可

原创代码使用 [MIT 许可](LICENSE)。第三方组件和下载的模型保留各自的条款，见 [第三方声明](THIRD_PARTY_NOTICES.zh-CN.md)。本项目是独立的个人项目，与 Formula 1、MultiViewer 和各翻译服务商无关。
