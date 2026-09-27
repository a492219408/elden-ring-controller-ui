use std::{fs, path::Path};

pub const DEFAULT_INI: &str = include_str!("../EldenRingControllerUI.ini");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ControllerLayout {
    #[default]
    DualSense,
    DualShock4,
    XboxSeries,
    Original,
}

impl ControllerLayout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DualSense => "dualsense",
            Self::DualShock4 => "dualshock4",
            Self::XboxSeries => "xbox_series",
            Self::Original => "xbox_one",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Config {
    pub layout: ControllerLayout,
    pub diagnostics: bool,
}

impl Config {
    pub fn load_or_create(path: &Path) -> Result<(Self, bool), String> {
        if !path.exists() {
            fs::write(path, DEFAULT_INI)
                .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
            return Ok((Self::default(), true));
        }

        let text = fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        Self::parse(&text).map(|config| (config, false))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let mut config = Self::default();
        let mut section = String::new();

        for (index, original_line) in text.lines().enumerate() {
            let line_number = index + 1;
            let line = original_line
                .trim_start_matches('\u{feff}')
                .split([';', '#'])
                .next()
                .unwrap_or_default()
                .trim();
            if line.is_empty() {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_ascii_lowercase();
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                return Err(format!("invalid INI syntax on line {line_number}"));
            };
            if section != "controllerui" {
                continue;
            }

            let key = key.trim();
            let value = value.trim().to_ascii_lowercase();
            if key.eq_ignore_ascii_case("layout") {
                config.layout = match value.as_str() {
                    "dualsense" | "ps5" => ControllerLayout::DualSense,
                    "dualshock4" | "dualshock" | "ps4" => ControllerLayout::DualShock4,
                    "xbox_series" | "xboxseries" | "series" => ControllerLayout::XboxSeries,
                    "xbox_one" | "xboxone" | "original" | "xbox" | "off" => {
                        ControllerLayout::Original
                    }
                    _ => {
                        return Err(format!(
                            "layout must be dualsense, dualshock4, xbox_series, or xbox_one (line {line_number})"
                        ));
                    }
                };
            } else if key.eq_ignore_ascii_case("diagnostics") {
                config.diagnostics = parse_bool("diagnostics", &value, line_number)?;
            }
        }

        Ok(config)
    }
}

fn parse_bool(key: &str, value: &str, line_number: usize) -> Result<bool, String> {
    match value {
        "true" | "yes" | "1" | "on" => Ok(true),
        "false" | "no" | "0" | "off" => Ok(false),
        _ => Err(format!("{key} must be true or false (line {line_number})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_dualsense_without_diagnostics() {
        let config = Config::parse("").unwrap();
        assert_eq!(config.layout, ControllerLayout::DualSense);
        assert!(!config.diagnostics);
    }

    #[test]
    fn accepts_aliases_comments_and_utf8_bom() {
        let config = Config::parse(
            "\u{feff}[ControllerUI]\nlayout = PS4 ; comment\noperating_lines=official\ndiagnostics=yes\nexperimental_atlas_hook=on\nexperimental_gfx_hook=off",
        )
        .unwrap();
        assert_eq!(config.layout, ControllerLayout::DualShock4);
        assert!(config.diagnostics);
    }

    #[test]
    fn ignores_future_keys_and_other_sections() {
        let config =
            Config::parse("[Other]\nlayout=bad\n[ControllerUI]\nfuture_value=42\nlayout=original")
                .unwrap();
        assert_eq!(config.layout, ControllerLayout::Original);
    }

    #[test]
    fn rejects_invalid_values() {
        assert!(Config::parse("[ControllerUI]\nlayout=switch").is_err());
        assert!(Config::parse("[ControllerUI]\ndiagnostics=perhaps").is_err());
    }

    #[test]
    fn obsolete_options_cannot_enable_removed_code() {
        let config = Config::parse("[ControllerUI]\noperating_lines=modified\nexperimental_atlas_hook=true\nexperimental_gfx_hook=true").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn four_styles_and_legacy_original_do_not_select_input_or_icon_mode() {
        for (name, layout) in [
            ("dualsense", ControllerLayout::DualSense),
            ("dualshock4", ControllerLayout::DualShock4),
            ("xbox_series", ControllerLayout::XboxSeries),
            ("xbox_one", ControllerLayout::Original),
            ("original", ControllerLayout::Original),
        ] {
            let config = Config::parse(&format!("[ControllerUI]\nlayout={name}\nforce_gamepad=true\ninput_mode=controller\nauto_detect=true")).unwrap();
            assert_eq!(
                config,
                Config {
                    layout,
                    diagnostics: false
                }
            );
        }
    }
}
