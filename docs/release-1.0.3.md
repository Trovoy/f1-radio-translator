# F1 Radio Translator v1.0.3

修复任务栏仍显示 PowerShell 图标的问题，适用于 Windows 10/11 x64。

## 本次修复

- 启动器与界面进程采用独立且一致的 Windows 应用标识。
- 在窗口显示前指定任务栏的赛车图标、产品名称和启动命令。
- 固定到任务栏时通过原始 EXE 启动应用。

## 下载与更新

- `F1-Radio-Translator-1.0.3.exe`：独立 EXE，内置 Python 和本地 OCR 依赖。
- `F1-Radio-Translator-1.0.3-Windows-x64.zip`：同一个 EXE、使用说明和许可证。
- `SHA256SUMS.txt`：两个下载文件的 SHA-256 校验值。

退出旧版后运行新版；密钥与选区仍使用本机原有设置。如曾固定旧版 PowerShell 图标，请取消旧固定项，再从新版窗口固定到任务栏。保留新版 EXE 所在位置，移动或删除它会影响固定项的启动路径。

打开 MultiViewer 的 AI Radio Transcriptions，框选卡片正文和姓名行，配置翻译服务后开始翻译。F8 切换简洁模式，F9 暂停/继续，F10 结束翻译（主窗口有焦点时）。双击托盘图标恢复窗口，点 X 退出应用。

本版完成源码检查与 EXE 编译；尚未在实际任务栏上验证图标显示。OCR、原始转录和机器翻译可能有误差。本项目与 F1、MultiViewer 无官方关联。

[源码](https://github.com/Trovoy/f1-radio-translator) · [构建说明](https://github.com/Trovoy/f1-radio-translator/blob/main/docs/build.md)
