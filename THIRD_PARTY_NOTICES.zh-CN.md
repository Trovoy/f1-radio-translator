[English](THIRD_PARTY_NOTICES.md) | **简体中文**

# 第三方组件说明

项目原创源码使用 MIT 许可证。第三方组件和模型仍适用各自的许可，不能以本项目的 MIT 许可证覆盖。

## 软件内置

| 组件 | 用途 | 许可 |
| --- | --- | --- |
| [Tauri](https://tauri.app/) 及其插件 | 窗口、托盘、快捷键、单实例 | MIT / Apache-2.0 |
| [windows-rs](https://github.com/microsoft/windows-rs) | Windows 文字识别、GDI 截屏、DPAPI、作业对象 | MIT / Apache-2.0 |
| [reqwest](https://github.com/seanmonstar/reqwest)、[tokio](https://tokio.rs/)、[serde](https://serde.rs/)、[regex](https://github.com/rust-lang/regex)、[zip](https://github.com/zip-rs/zip2) 等 `Cargo.lock` 中的库 | 网络、异步、解析、解压 | MIT / Apache-2.0 等宽松许可 |
| [Vite](https://vitejs.dev/) | 前端构建（不随软件分发） | MIT |
| Windows.Media.Ocr、WebView2 | 文字识别和网页视图 | 由 Windows 提供，本仓库不重新许可 |

## 按需下载

以下内容不在仓库和安装包里，只有在你选择本地模型时才会下载。

| 组件 | 来源 | 许可 |
| --- | --- | --- |
| [llama.cpp](https://github.com/ggml-org/llama.cpp) `llama-server` | GitHub Releases | MIT |
| NVIDIA CUDA 运行库（仅 CUDA 版） | llama.cpp GitHub Releases | NVIDIA CUDA EULA |
| [Qwen3 8B / 14B GGUF](https://huggingface.co/Qwen/Qwen3-8B-GGUF) | 魔搭社区 / Hugging Face | Apache-2.0 |
| [HY-MT1.5 7B / 1.8B GGUF](https://huggingface.co/tencent/HY-MT1.5-7B-GGUF) | 魔搭社区 / Hugging Face | 腾讯的模型许可，使用前请在模型页面阅读 |

耳机图标为本项目绘制，不使用官方 F1 或 MultiViewer 标识。F1、Formula 1、MultiViewer 以及车手和车队名称属于各自权利人；本项目是独立工具，与其无官方关联。
