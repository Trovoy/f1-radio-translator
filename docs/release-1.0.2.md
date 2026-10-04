# F1 Radio Translator v1.0.2

Windows x64 首个开源二进制发布，适用于 Windows 10/11。

## 下载

- `F1-Radio-Translator-1.0.2.exe`：独立 EXE，包含 Python 与本地 OCR 依赖。
- `F1-Radio-Translator-1.0.2-Windows-x64.zip`：包含同一个 EXE、使用说明及许可证；解压后运行。
- `SHA256SUMS.txt`：上述文件的 SHA-256 校验值。

两种下载方式使用相同程序。首次启动会在 `%LOCALAPPDATA%\MultiViewerRadioTranslator\app` 展开运行文件，之后复用缓存。翻译接口密钥需自行配置。

## 功能与修复

- 本地 OCR、英中双语译文与说话人标签。
- 可调整的字幕选区、放大预览与每 10 秒更新的缩略图。
- MyMemory、DeepL 和 OpenAI 兼容接口；GPT 选项使用 `gpt-6-luna`，上游须支持对应模型名称或别名。
- 手动简洁模式、密钥显示开关和本机加密保存。
- 暂停、结束与托盘控制；窗口 X 退出并清理工作进程。
- 修复 Windows PowerShell 5.1 下不可用的 `StandardInputEncoding` 设置，改用 UTF-8 字节管道写入。
- 修复构建脚本的 Python 版本检测参数引号兼容性。
- 从公开的脱敏源码构建，附 MIT 许可证和第三方组件说明。

## 使用

1. 运行 EXE，打开 MultiViewer 的 AI Radio Transcriptions。
2. 框选卡片正文与姓名行，确认预览区域。
3. 配置翻译服务和自己的密钥，点击“开始翻译”。

F8 切换简洁模式，F9 暂停/继续，F10 结束翻译（主窗口有焦点时）。双击托盘图标恢复窗口。

发布前完成源码脱敏检查、PowerShell 语法检查和 EXE 编译；未进行完整观赛场景运行验证。原始转录、OCR 和机器翻译可能有误差。项目与 F1、MultiViewer 无官方关联。

[源码与详细说明](https://github.com/Trovoy/f1-radio-translator) · [问题反馈](https://github.com/Trovoy/f1-radio-translator/issues)
