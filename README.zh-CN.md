# BreadShot

> Windows 上的零剪辑自动运镜录屏工具。

[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011-2563eb?logo=windows)](https://github.com/thomaslovebang/BreadShot/releases/latest)
[![Release](https://img.shields.io/github/v/release/thomaslovebang/BreadShot?include_prereleases)](https://github.com/thomaslovebang/BreadShot/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-22c55e)](LICENSE)

[English](README.md) · [下载安装包](https://github.com/thomaslovebang/BreadShot/releases/latest) · [报告问题](https://github.com/thomaslovebang/BreadShot/issues/new?template=bug_report.yml)

BreadShot 录制你的屏幕，理解鼠标操作，并自动生成带有平滑缩放和跟随镜头的 MP4。无需学习时间线，无需注册账号，无水印，也不需要先上传视频。

按下快捷键，完成演示，直接得到成片。

> BreadShot 原名 FocusFlow。我们在第一次正式公开推广前完成改名，希望它能以一个清晰、独立、适合长期发展的品牌继续成长。

## 为什么做 BreadShot

普通录屏软件只会交给你一段原始视频，专业视频编辑器则会再交给你一份剪辑工作。

BreadShot 希望把流程缩短成四步：

1. 选择显示器并按下全局快捷键。
2. 像平常一样演示产品、教程或问题。
3. 再次按下快捷键。
4. BreadShot 在本机分析操作、自动运镜并导出 MP4。

我们不准备把它做成另一个通用视频编辑器。我们的目标，是让大多数屏幕视频根本不需要手动剪辑。

## 当前已经实现

- 不限录制时长，无水印。
- 多显示器环境中选择一个显示器录制。
- 根据鼠标活动自动生成缩放和跟随镜头。
- 通过 Windows WASAPI 原生录制系统声音。
- 可选麦克风录制，并与系统声音同步混合。
- 自定义全局开始/结束快捷键，默认 `F8` 打开应用即可使用。
- Windows 17 种鼠标角色分别支持 `.cur` 与动画 `.ani` 文件。
- 按真实 Windows 指针基准进行全局等比例缩放。
- 显示器、保存位置、快捷键、声音、语言和鼠标设置均可持久保存。
- 本地处理，并在可用时使用 NVIDIA NVENC 加速。
- 成功处理后，保存目录只保留原始视频与自动运镜成片。
- 支持中文、英文、日语、法语、德语、西班牙语与阿拉伯语界面。

## 隐私优先

录制画面、鼠标数据、麦克风音频与自定义指针文件都留在你的电脑上。BreadShot 没有账号系统，录制和渲染也不依赖云服务。

## 产品方向

BreadShot 要成为一个真正的**自动导演**，而不是手动时间线编辑器。

下一个核心里程碑是 Automatic Director 2.0：

- 区分点击、双击、拖拽、滚动、输入与停留代表的不同意图。
- 把相关操作合并成稳定的镜头段落，不再对每个事件机械反应。
- 当判断置信度不足时，宁愿不放大，也不错误放大。
- 增加可靠的窗口录制与区域录制。
- 自动生成背景、边距、圆角和不同画幅构图。
- 让连续十次录制都稳定得让人感觉理所当然。

## 下载

从 [GitHub Releases](https://github.com/thomaslovebang/BreadShot/releases/latest) 下载最新 Windows 安装包。

安装包已经包含视频引擎，不需要另外安装 FFmpeg、Node.js 或 Rust。目前支持 64 位 Windows 10 和 Windows 11。公开 Alpha 版本暂未购买代码签名证书，因此 Windows SmartScreen 可能要求确认。

## 从源码运行

需要 Windows 10/11、Node.js 22+、Rust 工具链与 npm。

```powershell
cd apps/desktop
npm install
npm run prepare:ffmpeg
npm run tauri dev
```

检查 Rust 工作区：

```powershell
cargo check --workspace
```

构建 NSIS 安装包：

```powershell
cd apps/desktop
npm run bundle
```

## 参与贡献

BreadShot 还很年轻，并且有意保持清晰的产品边界。当前最有价值的贡献包括：可复现的录制失败、复杂的真实操作样本、不同 Windows 硬件兼容性报告，以及对自动导演算法的小而明确的改进。

提交功能建议前，请记住我们的产品原则：**只录一次，少做剪辑，更快发布。**

## 许可证

BreadShot 使用 [MIT License](LICENSE)。发布安装包中独立捆绑的 FFmpeg 可执行文件遵循 GPLv3，详情请参阅[第三方声明](THIRD_PARTY_NOTICES.md)。
