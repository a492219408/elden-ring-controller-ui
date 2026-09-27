//! 只替换文本内 21 个通用手柄图标名称；不选择键鼠/手柄显示模式，也不处理输入。
use crate::{config::ControllerLayout, windows};
use std::sync::atomic::{AtomicU64, Ordering};

pub const MAX_ENTRIES: usize = 21;
const MAX_NAME_UNITS: usize = 260;
const KG_SUFFIXES: &[&str] = &[
    "L1", "L2", "L3", "LS", "L_D", "L_L", "L_LR", "L_R", "L_U", "L_UD", "L_UDLR", "R1", "R2", "R3",
    "RS", "R_D", "R_L", "R_R", "R_U", "Start",
];
#[derive(Default)]
pub struct Statistics {
    pub calls: AtomicU64,
    pub failures: AtomicU64,
    pub max_hook_us: AtomicU64,
    pub max_game_us: AtomicU64,
}
impl Statistics {
    pub fn record(&self, failure: bool, hook_us: u64, game_us: u64) {
        self.failures
            .fetch_add(u64::from(failure), Ordering::Relaxed);
        self.max_hook_us.fetch_max(hook_us, Ordering::Relaxed);
        self.max_game_us.fetch_max(game_us, Ordering::Relaxed);
        self.calls.fetch_add(1, Ordering::Release);
    }
}
pub struct Entry {
    pub source: String,
    pub target: String,
    pub target_wide: Box<[u16]>,
    pub stats: Statistics,
}
pub struct RuntimeMappings {
    pub entries: Vec<Entry>,
    pub diagnostics: bool,
}
impl RuntimeMappings {
    pub fn new(layout: ControllerLayout, diagnostics: bool) -> Self {
        let mut result = Self {
            entries: Vec::new(),
            diagnostics,
        };
        let prefix = match layout {
            ControllerLayout::DualSense => "KG_PS5_",
            ControllerLayout::DualShock4 => "KG_PS4_",
            // Official Xbox Series key-guide crops equal the generic Xbox One entries.
            ControllerLayout::Original | ControllerLayout::XboxSeries => return result,
        };
        result.push("KG_Back", &format!("{prefix}TP"));
        for suffix in KG_SUFFIXES {
            result.push(&format!("KG_{suffix}"), &format!("{prefix}{suffix}"));
        }
        result
    }
    fn push(&mut self, source: &str, target: &str) {
        self.entries.push(Entry {
            source: source.to_owned(),
            target: target.to_owned(),
            target_wide: target
                .encode_utf16()
                .chain(Some(0))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            stats: Statistics::default(),
        });
    }
    pub fn decide(&self, input: *const u16) -> Option<usize> {
        let mut name = [0u16; MAX_NAME_UNITS];
        let length = windows::read_wide_name(input as usize, &mut name)?;
        let name = &name[..length];
        let start = name
            .iter()
            .rposition(|v| matches!(*v, 0x2f | 0x5c | 0x3a))
            .map_or(0, |i| i + 1);
        let basename = &name[start..];
        let end = basename
            .iter()
            .rposition(|v| *v == 0x2e)
            .unwrap_or(basename.len());
        let basename = &basename[..end];
        self.entries.iter().position(|entry| {
            basename.len() == entry.source.len()
                && basename
                    .iter()
                    .zip(entry.source.bytes())
                    .all(|(unit, byte)| *unit < 128 && (*unit as u8).eq_ignore_ascii_case(&byte))
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_twenty_one_generic_controller_names_are_redirected() {
        for layout in [ControllerLayout::DualSense, ControllerLayout::DualShock4] {
            let mappings = RuntimeMappings::new(layout, false);
            assert_eq!(mappings.entries.len(), MAX_ENTRIES);
            for name in [
                "KG_Key_A",
                "KG_Mouse_L",
                "KG_Mouse_R",
                "KG_Keyboard",
                "KG_PS5_L1",
                "KG_PS4_L1",
                "MENU_XboxOne",
                "MENU_Line_XboxOne",
                "MENU_Tab_Option",
                "MENU_Tab_Manual",
                "MENU_PS5_Trigger_Batt",
                "KG_L1_Extra",
                "",
            ] {
                let wide: Vec<_> = name.encode_utf16().chain(Some(0)).collect();
                assert!(mappings.decide(wide.as_ptr()).is_none(), "{name}");
            }
            let wide: Vec<_> = "menu:/foo/kg_l1.PNG"
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let entry = &mappings.entries[mappings.decide(wide.as_ptr()).unwrap()];
            assert_eq!(
                entry.target,
                if layout == ControllerLayout::DualSense {
                    "KG_PS5_L1"
                } else {
                    "KG_PS4_L1"
                }
            );
            assert!(mappings.decide(std::ptr::null()).is_none());
            assert!(mappings.decide(std::ptr::dangling::<u16>()).is_none());
        }
    }
    #[test]
    fn xbox_styles_do_not_need_texture_hooks() {
        for layout in [ControllerLayout::Original, ControllerLayout::XboxSeries] {
            assert!(RuntimeMappings::new(layout, false).entries.is_empty());
        }
    }
}
