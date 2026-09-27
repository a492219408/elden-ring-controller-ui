Elden Ring Native Controller UI 1.0.1
By Luna(a492219408) | GPL-3.0-only

One ModEngine3 DLL. Four controller UI layouts. No assets folder.

WHAT IT DOES
Selects the game's native DualSense, DualShock 4, Xbox Series or Xbox One
controller artwork, guide lines, button highlights, prompts and menu tab icons.
This is appearance-only: keyboard/mouse and controller input stay usable together.
The game's icon-display option remains under your control. It does not detect
controllers or add PlayStation input support, haptics or adaptive triggers.
Keep your existing Steam Input/controller compatibility setup.

REQUIREMENTS
Microsoft Visual C++ v14 x64 Redistributable (VCRUNTIME140.dll), not bundled:
https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist?view=msvc-170
Windows x64, ModEngine3, verified Elden Ring PC 1.17 / 1.17 or 1.17.1 / 1.17.1 with official UI.
Tested with ModEngine3 0.13.0 and DLC-present resources. Other game builds are
rejected; future updates are not automatically supported. Offline launch only.
Never load into an anti-cheat-protected session or official matchmaking.

INSTALLATION
1. Fully exit the game and extract this complete folder.
2. Load EldenRingControllerUI.me3 using ModEngine3.
   Existing profiles: copy its single [[natives]] entry, adjusting the DLL path
   relative to your profile. Keep load_early=true and the explicit initializer.
   Do not add a [[packages]] entry. Do not replace the game executable.
3. Edit natives/EldenRingControllerUI.ini if desired:
   layout = dualsense     (default, PS5)
   layout = dualshock4    (PS4)
   layout = xbox_series
   layout = xbox_one      (default game appearance, no game hooks)
   original is an alias for xbox_one.
4. Restart after changing layout. Select keyboard/controller prompt display in
   the game's own settings as usual. Mouse and keyboard remain usable either way.

Disable old Controller UI DLLs, DualSense/DualShock UI resource packs, and old
0.2.8 assets before use. Keep old INIs/releases separately if needed for rollback.
Other mods editing the same GFX/hook sites may conflict; mixed styling is possible.
No regulation, localization, save or input settings are modified by this DLL.

STATUS AND TROUBLESHOOTING
All four layouts were reported passing on game 1.17 in a normal multi-mod profile
using preview.5. Version 1.0.1 adds a separately verified game 1.17.1 address profile;
layout logic is unchanged. Both executable samples passed all 59 offline tests.
Visual testing on game 1.17.1 is pending; no game was launched during development.
This does not certify every mod/hardware combination.
Keep natives/EldenRingControllerUI.log BEFORE restarting, because each run replaces
it. Include layout, game version, icon setting, affected page and other UI mods.
The DLL folder must be writable. diagnostics=true enables timing statistics;
false still logs initialization and GFX results. There is no operating_lines option.
To uninstall, remove/disable the native entry after exiting the game.

LICENSE AND SOURCE
Copyright (C) 2026 Luna(a492219408). GNU GPL version 3 ONLY; no warranty.
See LICENSE.txt, NOTICE.txt and THIRD_PARTY_NOTICES.txt.
Matching source: elden-ring-controller-ui-1.0.1-source.zip, provided alongside
this download. Redistributors must also provide the matching corresponding source.
Game assets and trademarks are not covered by this project's code license.
