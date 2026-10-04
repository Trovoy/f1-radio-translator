# 第三方组件说明

项目原创源码使用 MIT 许可证。第三方组件、运行时和模型仍适用各自许可证，不能以本项目 MIT 许可证覆盖。

源码仓库不提交 Python 运行时、OCR 安装目录或模型权重。构建时通过 `requirements.txt` 安装依赖；打包 EXE 时保留运行时的 `LICENSE.txt` 和各 wheel 的 `.dist-info` / 许可证文件。

| 组件 | 用途 | 许可或说明 |
| --- | --- | --- |
| [Python](https://docs.python.org/3/license.html) | OCR 运行时 | PSF 及其包含的第三方许可；构建时保留运行时 LICENSE.txt |
| [RapidOCR](https://pypi.org/project/rapidocr/) | OCR 管线 | Apache-2.0；实际模型及数据文件按上游随包声明处理 |
| [ONNX Runtime](https://pypi.org/project/onnxruntime/) | 模型推理 | MIT；随包第三方通知需要保留 |
| OpenCV、NumPy、Pillow、Shapely 等 | 图像处理与依赖 | 保留各安装包的许可及元数据 |
| Windows PowerShell / WPF / .NET Framework | Windows 窗口与进程管理 | 使用 Windows 环境提供的组件；本仓库不重新许可这些组件 |

项目赛车图标为本项目生成的图形资产，不使用官方 F1 或 MultiViewer 标识。F1、Formula 1、MultiViewer 等名称属于其各自权利人；本项目是独立工具，与其无官方关联。
