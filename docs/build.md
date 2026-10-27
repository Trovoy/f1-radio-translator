**English** | [简体中文](build.zh-CN.md)

# Build and run

## Requirements

- 64-bit Windows 10/11, Windows PowerShell 5.1 and the Windows-provided .NET Framework/WPF.
- A **full 64-bit Python 3.12 installation**, including `python.exe`, `python312.dll`, `Lib/`, `DLLs/` and license files.
- Access to the Python package index for initial OCR dependency installation.

No Codex installation, personal development path or author account configuration is required.

## Prepare dependencies

From the repository root:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Setup-Dependencies.ps1
```

Select a Python installation explicitly if needed:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Setup-Dependencies.ps1 -PythonExe "D:\Python312\python.exe"
```

Dependencies are installed in `src/python-deps/`. Source runs load the C# region selector from source; EXE builds precompile it as a DLL.

## Build a single-file EXE

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\Build-Executable.ps1 -PythonExe "D:\Python312\python.exe"
```

Alternatively pass `-PythonDirectory "D:\Python312"` for a full runtime directory. By default the script looks for Python 3.12 installations/Python Launcher and writes `dist/F1-Radio-Translator-1.0.2.exe`. Use `-OutputDirectory` to choose another output directory.

The build creates multi-size icons, collects source/OCR dependencies, compiles the region selector, gathers Python, compresses the payload and compiles a console-free EXE. The launcher uses Windows-provided PowerShell/.NET Framework.

`dist/`, dependency folders, DLL/EXE/ZIP outputs, logs and local settings are ignored by Git. Distribute binaries through Releases rather than committing them. Preserve model and third-party licenses with binaries; the project's MIT license does not replace them.

## Runtime data

| Data | Location |
| --- | --- |
| User settings | `%LOCALAPPDATA%\MultiViewerRadioTranslator\settings.json` |
| EXE dependency cache | `%LOCALAPPDATA%\MultiViewerRadioTranslator\app\<payload-version>` |

Different payloads use different cache identifiers. No extraction directory is created beside the EXE. A Windows Job Object reclaims launcher-owned child processes when the main process exits.

## Validation before release

The source is based on 1.0.2 and includes the Windows PowerShell 5.1 stdin-encoding compatibility fix. Source cleanup/build success is not a full runtime acceptance test. Verify OCR, each intended translation provider and tray/window exit on the target Windows system before publishing binaries.

[Back to README](../README.md) · [Privacy](privacy.md)
