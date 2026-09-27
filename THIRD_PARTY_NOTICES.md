# 许可、第三方与游戏素材

Elden Ring Native Controller UI 的原创源码和随附文档：
Copyright (C) 2026 Luna(a492219408)，SPDX-License-Identifier: GPL-3.0-only。
仅 GPL 第 3 版，不是 GPL-3.0-or-later。完整条款见 LICENSE，项目适用声明见 NOTICE。
旧本地版本保留其原许可与历史，不追溯改写已发布的授权；本次也未修改其他项目的许可。

## 源码与依赖

Cargo.lock 只有本项目，无第三方 crate 依赖。运行时使用 Rust 标准库及 Windows 系统 API。
Rust 标准库遵循 Rust 项目的 MIT/Apache-2.0 等上游声明；系统组件遵循其各自许可，
本项目的 GPL 声明不改变这些组件的授权。
随包保留 Rust 1.96.0 工具链提供的完整标准库版权与许可声明：
源码 third_party/Rust-COPYRIGHT-library.html；运行包 Rust-COPYRIGHT-library.html。
文件含上游完整许可与各组件声明，覆盖面可能大于本 Windows DLL 实际链接的组件。

ModEngine3 为外部加载器，用户自行安装；本包不包含其二进制或源码。
Ghidra、WitchyBND、Nuxe、FFDec 仅用于开发分析，不是本 MOD 的运行依赖，也不随包分发。
未复制原 DualSense UI / DualShock UI 资源包的修改纹理或文本到本发布包。

## 游戏素材和截图

Elden Ring 的纹理、GFX、图标、名称及其他游戏内容属于各自权利人。
本项目不拥有、也不通过 GPL 重新许可这些游戏素材。
运行时从用户合法安装的游戏中读取已校验界面，在内存中选择或生成原生布局；
不修改磁盘上的 EXE、regulation、语言文本、纹理或存档。

三份官方 GFX 只作为维护者自备、明确启用的 game-fixtures 测试输入。
它们不在公开源码、玩家 ZIP 或 Release DLL 中。开发测试可以显式输出临时 GFX；
发布 DLL 无此落盘功能。最小布局参数和图像标识只用于实现互操作，不包含纹理像素或完整电影。

Nexus 宣传截图为维护者提供的实机截图；封面是基于这些截图的 AI 辅助宣传合成，
不是一张未经编辑的游戏截图。截图及封面中的游戏图像、控制器商标不属于 GPL 代码授权。
实际效果证据应以发布页面中未编辑的实机截图为准。宣传素材不随公开源码包分发。

## 源码分发

请把同版本 elden-ring-controller-ui-<版本>-source.zip 与运行包一并提供下载，
并在下载页面给出明确的对应源码入口。源码包包含修改、构建和安装所需的原创文件与脚本，
可在不持有测试 GFX 的情况下构建 DLL。无须将私有 Git 历史或官方样本上传。

许可文本来源：[SPDX GPL-3.0-only](https://spdx.org/licenses/GPL-3.0-only.html)。
Rust 的上游说明：[Rust copyright](https://www.rust-lang.org/policies/licenses)。
