# Elden Ring Native Controller UI 1.0.1

Game 1.17.1 compatibility update (2026-09-27).

- Adds an independent, exact executable profile for App Ver. 1.17.1 / Calibrations Ver. 1.17.1 (file version 2.7.1.0), while retaining 1.17 support.
- Updates the moved texture resolvers, native file opener/constructor, reference-count function and both File method tables. The two tab-selector sites, heap slot and vtable locations remain unchanged.
- Keeps full executable fingerprints, PE/prologue/vtable checks, GFX input/output hashes and rollback checks. Unknown builds are still rejected; no bypass or guessed future-version support.
- Logs the selected executable profile. No new assets, layout changes, input hooks or configuration options.

Both real executable samples pass all 59 offline tests; the public asset-free suite passes 26 tests. The seven relevant UI archive entries are unchanged, and all eight generated UI outputs match 1.0.0. Game 1.17.1 visual regression testing is still pending; no game was launched during this update.

Download `EldenRingControllerUI-1.0.1.zip` with its checksum. Matching GPL-3.0-only source is `elden-ring-controller-ui-1.0.1-source.zip`. Keep your INI settings. Exit the game before replacing the DLL; do not load both versions at once. No `assets/` entry is needed.

简体中文：为游戏 1.17.1 增加经验证的独立函数地址配置，继续支持 1.17；不是简单放宽哈希。
界面资源与布局逻辑保持不变，两版离线测试通过，1.17.1 的实机画面仍需用户回归。
升级可保留原 INI。安装与构建方法见 README.zh-CN.md。

## Elden Ring Native Controller UI 1.0.0

First public release. By **Luna(a492219408)**. Licensed under **GPL-3.0-only**.

## Highlights

- One ModEngine3 DLL and one INI for DualSense, DualShock 4, Xbox Series and Xbox One UI.
- Native controller artwork, overview guide lines, menu tab icons and button-highlighting animations.
- No assets folder, bundled textures, localization files, regulation changes or embedded full GFX movies.
- Appearance only: the game's icon-display preference and simultaneous keyboard/controller input remain unchanged.
- No device detection, input emulation, extra haptics or adaptive trigger support.

## Verification and scope

All four layouts in `0.3.0-preview.5` were reported passing in the maintainer's normal multi-mod profile on 2026-09-06. The release retains that UI logic. Final checks include 23 asset-free public tests, the complete 56-test official-sample suite, eight independent GFX/XML output comparisons, package inspection and a clean-source rebuild. The final release binary was not separately game-launched by the developer.

Only the verified PC App Ver. 1.17 / Calibrations Ver. 1.17 executable and official UI are supported. The tested loader is ModEngine3 0.13.0; the test installation contains the DLC. This is not universal compatibility with other UI mods or future game builds. Offline MOD launch only, not official matchmaking.

## Downloads

- `EldenRingControllerUI-1.0.0.zip`: runtime DLL, INI, ME3 profile and bilingual instructions.
- `elden-ring-controller-ui-1.0.0-source.zip`: matching GPL corresponding source, without game assets or private Git history.
- Matching `.sha256` files for both archives.

Load the included `EldenRingControllerUI.me3` or copy its native entry into your existing profile. Do not add a `packages` entry. Default `layout = dualsense`; restart after editing the INI. Disable older controller UI DLLs and resource packs before installing.

Version 1.0.0 also adds explicit initialization-failure logging, public tests that do not require proprietary fixtures, license/source-distribution checks, and publication materials. Game UI selection, file lifetime and input behavior are unchanged from the successful preview.

## 简体中文

首个正式版。四种布局已由维护者在常用多 MOD 配置中确认通过；正式版沿用 preview.5 的界面逻辑，
完成离线审计和源码独立构建，没有重新启动游戏。默认 DualSense，不需要 assets；修改布局后重启。
运行包与同版本 GPL-3.0-only 对应源码包请一并提供下载。详情见中文 README。
