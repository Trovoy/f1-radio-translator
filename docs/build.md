# 构建说明

## 环境

- 64 位 Windows 10/11。
- Windows PowerShell 5.1 和 Windows 提供的 .NET Framework / WPF。
- 64 位 Python 3.12 的完整安装目录，内含 `python.exe`、`python312.dll`、`Lib/`、`DLLs/` 和许可证文件。
- 首次准备 OCR 依赖需要访问 Python 包索引。

本项目不要求安装 Codex，也没有个人开发目录或账户配置依赖。

## 准备依赖

在仓库根目录运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Setup-Dependencies.ps1
```

指定 Python 安装位置：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Setup-Dependencies.ps1 -PythonExe "D:\Python312\python.exe"
```

脚本将 OCR 依赖装到 `src/python-deps/`。运行源码时区域选择器会从 C# 源码加载；构建 EXE 时会预编译为 DLL。

## 构建单文件 EXE

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build-Executable.ps1 -PythonExe "D:\Python312\python.exe"
```

也可使用 `-PythonDirectory "D:\Python312"` 提供完整运行时目录。默认从 Python 3.12 安装位置或 Python Launcher 查找，输出在 `dist/F1-Radio-Translator-1.0.3.exe`。`-OutputDirectory` 可修改产物位置。

构建流程：生成多个尺寸的 ICO → 收集源码和 OCR 依赖 → 编译区域选择器与任务栏集成模块 → 收集 Python 运行时 → 压缩本地载荷 → 编译无控制台 EXE。启动器只使用 Windows 提供的 PowerShell / .NET Framework。

`dist/`、依赖目录、DLL、EXE、ZIP、日志和本机配置均在 `.gitignore` 中。EXE 不提交进 Git；通过 GitHub Releases 分发。模型权重与第三方许可证需要随二进制保留，不由项目 MIT 许可证覆盖。

## 运行时缓存

用户设置：`%LOCALAPPDATA%\MultiViewerRadioTranslator\settings.json`。

EXE 依赖缓存：`%LOCALAPPDATA%\MultiViewerRadioTranslator\app\版本标识\`。不同载荷使用不同标识，EXE 旁不生成解压文件夹。启动器通过 Windows Job Object 管理所属工作进程，主进程退出时回收子进程。

## 当前发布状态

源码整理自 1.0.2 实现，包含对 Windows PowerShell 5.1 输入编码属性的兼容性修复。源码整理本身不等于完整运行验证；发布二进制前应在目标 Windows 环境确认 OCR、翻译接口和托盘退出流程。
