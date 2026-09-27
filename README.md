# Elden Ring Native Controller UI

[简体中文](README.zh-CN.md) · Version 1.0.1 · By Luna(a492219408)

Choose **DualSense, DualShock 4, Xbox Series, or Xbox One** controller UI with one Rust DLL and an INI file, loaded through ModEngine3. Uses the game's own controller artwork, button highlights and UI presets. No `assets/` folder or replacement texture/text archives.

## What changes

- Controller button prompts, the controller overview and its guide lines.
- Controller images and native button-highlighting animations in Button Settings.
- The Game Options and Controller Settings tab icons.

This changes **appearance only**. Keyboard/mouse and controller input remain simultaneously usable. The game's own keyboard/controller icon-display option stays in control. No controller detection, input emulation, native PlayStation input support, adaptive triggers or new haptics. Keep your existing Steam Input/controller compatibility setup.

## Requirements and compatibility

Windows x64, ModEngine3, and a verified Elden Ring PC 1.17 / 1.17 or 1.17.1 / 1.17.1 build with official UI resources. Tested loader baseline: ModEngine3 0.13.0. Each executable has its own exact fingerprint and code-address profile. Unknown executable hashes are rejected; future game updates are **not** automatically supported.

The DLL imports `VCRUNTIME140.dll`; install the [Microsoft Visual C++ v14 x64 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170) if it is missing. The runtime is not bundled.

The maintainer reported all four layouts passing on game 1.17 in the normal multi-mod profile on 2026-09-06, using `0.3.0-preview.5`. Version 1.0.1 adds game 1.17.1 support after checking its moved functions and unchanged UI archives; visual testing on the new game version is **pending**. Layout logic is unchanged. Supported executable fingerprints and address profiles are defined in `src/compat.rs`.

Use ModEngine3's offline launch flow. Do not load this DLL into an anti-cheat-protected session or use it in official matchmaking.

## Installation

1. Fully exit the game. Extract `EldenRingControllerUI-1.0.1.zip` and keep its folder structure.
2. Load the included `EldenRingControllerUI.me3` with ModEngine3, or add its single native entry to your existing profile and adjust the DLL path relative to that profile.
3. Keep `load_early = true` and `initializer = { function = "elden_ring_controller_ui_init" }`. Do not add a resource `packages` entry.
4. Edit `natives/EldenRingControllerUI.ini` if desired; the default is `layout = dualsense`.
5. Launch through ModEngine3. Choose keyboard or controller prompt display in the game as usual.

| `layout` | Appearance |
|---|---|
| `dualsense` | DualSense / PS5 |
| `dualshock4` | DualShock 4 / PS4 |
| `xbox_series` | Xbox Series |
| `xbox_one` | Default Xbox One; no game hooks installed |

`original` remains an alias for `xbox_one`. Restart the game after editing `layout`; no hot switching. Remove/disable old Controller UI DLLs and old DualSense/DualShock UI resource packs, including the old 0.2.8 assets. Do not overwrite their INIs if you want to preserve a rollback copy.

Other mods changing the same two GFX movies or hook sites can conflict. A foreign movie is left untouched, but tab/prompt selection is independent and mixed styling is possible. No changes to regulation, localization, gameplay, save files or Steam settings.

## Troubleshooting

Keep `natives/EldenRingControllerUI.log` before restarting: it is replaced each run. Include the chosen layout, game version, icon-display setting, affected menu and other UI mods. `diagnostics = true` enables additional timing statistics; `false` still records initialization and GFX results. The DLL directory must be writable for its INI/log. There is no `operating_lines` option.

## Build and test

Windows x64 with Rust 1.96.0 and the MSVC/Windows SDK toolchain. Python 3.11+ is needed for release verification. Development tools use `mise exec --` when mise is available; Rust need not be managed by mise. CI pins Rust 1.96.0.

```powershell
./scripts/build.ps1
./scripts/check-source.ps1
./scripts/package.ps1 -SkipBuild
./scripts/package-source.ps1
./scripts/verify-package.ps1 -PackageDirectory dist/EldenRingControllerUI-1.0.1 -SourceArchive dist/elden-ring-controller-ui-1.0.1-source.zip
```

Public builds and the default 26 Rust tests do not need game files. Source-policy tests and packaging checks use Python 3.11+. Packaging from a Git checkout requires a clean, committed tree containing exactly the files in `packaging/source-files.txt`. Existing archives are never overwritten; use a fresh output directory or increment the version. The source ZIP can also build and package independently, without Git or any other repository.

Optional fixture tests require a legally obtained, read-only EXE and three hash-verified GFX inputs. Run `scripts/verify-samples.ps1` with `-EldenRingExe`, `-OfficialOptionsGfx` (`menu/win/02_040_optionsetting.gfx`), `-CommonOptionsGfx` (`menu/02_040_optionsetting.gfx`), and `-KeyConfigurationGfx` (`menu/02_160_keyconfiguration.gfx`). These inputs enable the complete 59-test suite; run separately for each supported EXE. No game is launched. Fixtures are not distributed or needed for a normal build. The checked-in `src/overview_data.rs` is sufficient to build; its optional regeneration tool, `tools/generate-overview-presets.py`, accepts an explicitly supplied common GFX sample and prints Rust definitions to standard output.

## License and releases

Copyright (C) 2026 **Luna(a492219408)**. **GPL-3.0-only**, not “or later”; see [LICENSE](LICENSE), [NOTICE](NOTICE), and [third-party/game-material notices](THIRD_PARTY_NOTICES.md). No warranty.

Distribute the matching `elden-ring-controller-ui-1.0.1-source.zip` alongside the runtime archive. See [release notes](RELEASE_NOTES.md). This repository contains product source, build and verification tools, and essential user/license documentation only. Local AI instructions, research notes, promotional drafts, game data and private history are excluded from both Git and source packages.
