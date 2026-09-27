Elden Ring Native Controller UI 1.0.1
作者 Luna(a492219408) | GPL-3.0-only

一个 ModEngine3 DLL，四种手柄界面外观，不需要 assets 文件夹。

功能与边界
选择游戏原生 DualSense、DualShock 4、Xbox Series 或 Xbox One 的大图、引线、
按键高亮、提示图标和菜单页签。仅影响显示；键鼠和手柄仍能同时使用。
游戏内的图标显示开关仍由你控制，不自动识别设备，不增加 PS 原生输入、震动
或自适应扳机支持，继续使用已有 Steam Input 等兼容方案。

要求
Microsoft Visual C++ v14 x64 运行库（VCRUNTIME140.dll），本包不附带：
https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170
Windows x64、ModEngine3、已校验的 PC 1.17 / 1.17 或 1.17.1 / 1.17.1 游戏与官方 UI。
实测为 ModEngine3 0.13.0、有 DLC 资源；没有无 DLC 单独回归结论。
其他 EXE 指纹会拒绝安装 Hook，未来版本不自动兼容。
仅通过离线 MOD 流程启动，不加载到反作弊保护会话中，不用于官方匹配。

安装
1. 完全退出游戏，解压完整文件夹。
2. 使用 ModEngine3 加载 EldenRingControllerUI.me3。
   已有整合配置可复制其中唯一的 [[natives]] 条目，按配置文件位置调整 DLL 路径，
   保留 load_early=true 和显式 initializer。不要增加 [[packages]]，不要替换游戏 EXE。
3. 按需编辑 natives/EldenRingControllerUI.ini：
   layout = dualsense     默认，PS5
   layout = dualshock4    PS4
   layout = xbox_series
   layout = xbox_one      游戏默认外观，不安装 Hook
   original 为 xbox_one 的兼容别名。
4. 修改 layout 后重启。游戏内仍可自行选择显示键鼠或手柄提示，两类输入始终可用。

先禁用旧版 DLL、DualSense/DualShock UI 资源包和 0.2.8 assets，避免同时加载。
旧 INI、旧发布包可另外保留以便回退。改同一 GFX 或 Hook 位置的 MOD 可能冲突，
页签/名称选择与 GFX 交付独立，因此冲突时可能出现混合外观。
本 DLL 不修改 regulation、语言文本、玩法、存档或输入设置。

验证与排错
维护者已确认 preview.5 的四种布局在游戏 1.17 的常用多 MOD 配置中通过。
1.0.1 为游戏 1.17.1 增加独立地址配置，两版各 59 项样本测试均通过，布局逻辑不变。
新版本游戏的实机画面仍待用户确认；开发者本轮没有启动游戏。
这不是所有 MOD 或硬件组合的兼容保证。
重启前保留 natives/EldenRingControllerUI.log，每次启动会覆盖该日志。
反馈请附 layout、游戏版本、图标显示选项、具体页面和其他 UI MOD。
DLL 所在目录须可写；diagnostics=true 增加耗时统计，false 仍记录基础结果。
没有 operating_lines 开关。卸载时退出游戏后移除/禁用 native 条目即可。

许可与源码
Copyright (C) 2026 Luna(a492219408)。仅 GNU GPL 第 3 版，不提供任何担保。
参见 LICENSE.txt、NOTICE.txt、THIRD_PARTY_NOTICES.txt。
源码在 GitHub 公开仓库：https://github.com/a492219408/elden-ring-controller-ui
请选择与 MOD 版本对应的标签，本版为 v1.0.1。Nexus Mods 不上传源码压缩包。
再分发 DLL 时请保持对应源码可获取。游戏素材及商标不属于本项目 GPL 代码授权。
