# Elden Ring Native Controller UI

[English](README.md) · 1.0.1 · 作者 Luna(a492219408)

一个 Rust DLL，通过 ModEngine3 加载，在 INI 中选择 DualSense、DualShock 4、Xbox Series 或 Xbox One 外观。
使用游戏自己的手柄图像、引线、按钮高亮和界面预设，不需要 assets/ 或整套纹理/语言资源替换。

## 功能与边界

- 切换文字中的按键提示、控制器配置总览的大图与引线。
- 切换“按键配置”的手柄大图和原生按键高亮动画。
- 切换“游戏选项”“控制器配置”两个页签图标。

只改变显示，不改变按键功能。键鼠与手柄仍可同时使用，显示哪类按键图标仍由游戏内设置决定。
没有自动识别设备、输入模拟、PS 原生输入、额外震动或自适应扳机功能；继续使用已有的 Steam Input 等兼容方案。

## 支持范围

Windows x64、ModEngine3，以及已校验的 PC 1.17 / 1.17 或 1.17.1 / 1.17.1 和官方 UI 资源。
两种 EXE 分别使用独立的精确指纹与代码地址配置，不把旧地址直接用于新版。
实测加载器为 ModEngine3 0.13.0。EXE 指纹不符时拒绝安装 Hook，未来版本不自动兼容。
DLL 导入 VCRUNTIME140.dll；缺少时安装 [Microsoft Visual C++ v14 x64 运行库](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170)，本包不附带系统运行库。
本次使用有 DLC 的游戏资源；没有据此声称已完成无 DLC 环境回归。

维护者于 2026-09-06 确认 preview.5 的四种布局在游戏 1.17 的常用多 MOD 配置中通过。
1.0.1 重新验证并适配了 1.17.1 的函数地址，布局逻辑不变；已完成两版离线回归，
新游戏版本的实机画面仍待用户确认。本轮没有启动游戏，不扩大为所有 MOD/硬件组合的保证。
支持的 EXE 指纹与独立地址配置见 `src/compat.rs`。

仅通过 ModEngine3 的离线流程使用，不加载到反作弊保护的会话中，不用于官方匹配。

## 安装与配置

1. 完全退出游戏，解压 EldenRingControllerUI-1.0.1.zip 并保留目录结构。
2. 用 ModEngine3 加载 EldenRingControllerUI.me3；已有整合配置可复制其中唯一的 native 条目，按该配置的位置调整 DLL 路径。
3. 保留 load_early=true 和显式 initializer；不要增加资源 packages 条目。
4. 编辑 natives/EldenRingControllerUI.ini，默认 layout=dualsense。
5. 通过 ModEngine3 启动，游戏内的图标显示设置仍按自己的习惯选择。

| layout | 外观 |
|---|---|
| dualsense | DualSense / PS5 |
| dualshock4 | DualShock 4 / PS4 |
| xbox_series | Xbox Series |
| xbox_one | Xbox One 默认外观，不安装游戏 Hook |

original 仍是 xbox_one 的兼容别名。修改 layout 后必须重启，不热切换。
禁用旧 DLL、旧 DualSense/DualShock UI 资源包及 0.2.8 assets，不要同时加载。
旧 INI、发布包可保留用于回退。

若其他 MOD 修改相同的 GFX 或 Hook 位置，可能冲突。未知 GFX 原样返回，但页签/按键名称选择独立，
因此不能保证叠加后样式一致。不修改 regulation、文本、玩法、存档或 Steam 设置。

## 排错

保留新版 natives/EldenRingControllerUI.log 后再重启，因为日志每次会覆盖。
反馈请附布局、游戏版本、游戏图标设置、异常页面和其他 UI MOD。
diagnostics=true 可增加耗时统计，false 仍有初始化与 GFX 结果日志。
DLL 所在目录须可写，以保存 INI/日志；operating_lines 功能已移除。

## 构建与许可

需要 Windows x64、Rust 1.96.0、MSVC/Windows SDK；发布检查还需 Python 3.11+。
优先使用 mise 环境；CI 固定 Rust 1.96.0，不强制改动本机 SDK 配置。

```powershell
./scripts/build.ps1
./scripts/check-source.ps1
./scripts/package.ps1 -SkipBuild
./scripts/package-source.ps1
./scripts/verify-package.ps1 -PackageDirectory dist/EldenRingControllerUI-1.0.1 -SourceArchive dist/elden-ring-controller-ui-1.0.1-source.zip
```

默认 26 项 Rust 测试和 Release 构建不需要游戏资源。源码边界测试、检查与打包使用 Python 3.11+。
在 Git 工作区中打包要求已提交、干净的状态，跟踪文件必须与 `packaging/source-files.txt` 白名单完全一致。
已有发布包不会被覆盖；重新打包请使用新输出目录或新版本号。源码 ZIP 解压后也能独立构建、打包，
不依赖 Git、其他仓库或私有工具。

可选的完整 59 项测试需要自行合法取得的只读 EXE 与三份精确 GFX，调用 `scripts/verify-samples.ps1`：
`-EldenRingExe` 指定 EXE；`-OfficialOptionsGfx` 指定 `menu/win/02_040_optionsetting.gfx`；
`-CommonOptionsGfx` 指定 `menu/02_040_optionsetting.gfx`；`-KeyConfigurationGfx` 指定
`menu/02_160_keyconfiguration.gfx`。两版 EXE 分别运行，脚本不会启动游戏。
这些样本不随源码分发，也不是正常构建依赖。已提交的 `src/overview_data.rs` 足以参与编译；
需要重新生成最小布局描述时，公开工具 `tools/generate-overview-presets.py` 接收显式指定的通用 GFX 样本，
向标准输出打印 Rust 定义，不生成完整游戏资源。

公开仓库只保存产品源码、构建／验证工具及必要使用和许可说明。AI 工作说明、研究文档、宣传草稿、
游戏数据及旧私有历史不进入 Git 或对应源码 ZIP。

Copyright (C) 2026 Luna(a492219408)。[GPL-3.0-only](LICENSE)，只许可 GPL 第 3 版，不是 or-later；
参见 [NOTICE](NOTICE) 和 [第三方/游戏素材说明](THIRD_PARTY_NOTICES.md)。不提供任何担保。
发布 DLL 时应同时提供同版本对应源码包，不以游戏素材充当 GPL 源码的一部分。
