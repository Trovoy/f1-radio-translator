**English** | [简体中文](THIRD_PARTY_NOTICES.zh-CN.md)

# Third-party notices

Original project source is MIT licensed. Third-party components, runtimes and models retain their own terms; the project's MIT license does not replace them.

Python runtimes, installed OCR dependencies and model weights are not committed. The build installs dependencies from `requirements.txt`; packaged EXEs retain runtime `LICENSE.txt` files and wheel `.dist-info`/license files.

| Component | Use | Terms / handling |
| --- | --- | --- |
| [Python](https://docs.python.org/3/license.html) | OCR runtime | PSF and included third-party terms; retain runtime LICENSE.txt. |
| [RapidOCR](https://pypi.org/project/rapidocr/) | OCR pipeline | Apache-2.0; follow upstream notices for actual model/data files. |
| [ONNX Runtime](https://pypi.org/project/onnxruntime/) | Model inference | MIT; retain bundled third-party notices. |
| OpenCV, NumPy, Pillow, Shapely and dependencies | Image processing | Retain each installed package's license and metadata. |
| Windows PowerShell / WPF / .NET Framework | Windows UI/processes | Supplied by Windows; not relicensed by this repository. |

The racing icon was generated for this project and does not use the official F1 or MultiViewer marks. F1, Formula 1, MultiViewer and other names belong to their respective holders. This independent tool is not officially affiliated with them.
