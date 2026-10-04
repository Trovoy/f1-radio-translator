# Changelog

## 1.0.2 — 开源整理

- 保留屏幕 OCR、英中双语、姓名标签、F1 语境和多翻译服务实现。
- 保留区域拖拽手柄、放大预览、10 秒缩略图更新、简洁模式与托盘控制。
- Windows PowerShell 5.1 下使用 UTF-8 管道字节写入，替代不可用的 `StandardInputEncoding` 属性。
- EXE 启动器使用 Job Object 管理工作进程退出。
- 整理公开源码，移除开发环境路径，补充通用构建脚本、MIT 许可证和产品详情页。
- 修复 Windows PowerShell 5.1 下构建脚本的 Python 版本检测参数引号兼容性。
- 提供 Windows x64 EXE、便携 ZIP 和 SHA-256 校验文件。
- 不提交本机配置、凭证、截图、运行依赖或生成的二进制。
