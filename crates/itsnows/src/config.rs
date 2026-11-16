//! User settings: what is stored in `settings.json` and how it is read back.
//! Reading never fails; anything missing or malformed falls back to its default.

use serde_json::{json, Value};
use snow_core::{LightsMode, LightsPattern, Settings};

pub const PRESETS: [(&str, f32); 5] = [("Off", 0.0), ("Light", 0.25), ("Medium", 0.5), ("Heavy", 0.75), ("Blizzard", 1.0)];
pub const LIGHTS_MODES: [(&str, &str); 3] = [("Off", "off"), ("Top of screen", "screen_top"), ("On windows", "windows")];
pub const LIGHTS_PATTERNS: [&str; 3] = ["steady", "twinkle", "chase"];

#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    /// 0..=1
    pub intensity: f32,
    /// "off" | "screen_top" | "windows"
    pub lights_mode: String,
    /// "steady" | "twinkle" | "chase"
    pub lights_pattern: String,
    pub show_in_capture: bool,
    pub paused: bool,
    /// Global hotkeys are registered. Off for people whose keyboard layout or apps need those keys.
    pub hotkeys: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { intensity: 0.5, lights_mode: "off".into(), lights_pattern: "twinkle".into(), show_in_capture: false, paused: false, hotkeys: true }
    }
}

impl AppSettings {
    pub fn parse(text: &str) -> Self {
        let mut s = Self::default();
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else { return s };
        if let Some(v) = map.get("intensity").and_then(Value::as_f64) {
            if v.is_finite() {
                s.intensity = (v as f32).clamp(0.0, 1.0);
            }
        }
        if let Some(v) = map.get("lights_mode").and_then(Value::as_str) {
            if LIGHTS_MODES.iter().any(|(_, key)| *key == v) {
                s.lights_mode = v.to_string();
            }
        }
        if let Some(v) = map.get("lights_pattern").and_then(Value::as_str) {
            if LIGHTS_PATTERNS.contains(&v) {
                s.lights_pattern = v.to_string();
            }
        }
        if let Some(v) = map.get("show_in_capture").and_then(Value::as_bool) {
            s.show_in_capture = v;
        }
        if let Some(v) = map.get("paused").and_then(Value::as_bool) {
            s.paused = v;
        }
        if let Some(v) = map.get("hotkeys").and_then(Value::as_bool) {
            s.hotkeys = v;
        }
        s
    }

    pub fn to_json(&self) -> String {
        let v = json!({
            "intensity": self.intensity,
            "lights_mode": self.lights_mode,
            "lights_pattern": self.lights_pattern,
            "show_in_capture": self.show_in_capture,
            "paused": self.paused,
            "hotkeys": self.hotkeys,
        });
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    pub fn to_core(&self) -> Settings {
        Settings {
            intensity: self.intensity,
            lights_mode: match self.lights_mode.as_str() {
                "screen_top" => LightsMode::ScreenTop,
                "windows" => LightsMode::Windows,
                _ => LightsMode::Off,
            },
            lights_pattern: match self.lights_pattern.as_str() {
                "steady" => LightsPattern::Steady,
                "chase" => LightsPattern::Chase,
                _ => LightsPattern::Twinkle,
            },
        }
    }

    /// Change intensity by `delta`, staying on a 0.05 grid inside 0..=1.
    pub fn step_intensity(&mut self, delta: f32) {
        let v = (self.intensity + delta).clamp(0.0, 1.0);
        self.intensity = (v * 20.0).round() / 20.0;
    }

    pub fn next_pattern(&mut self) {
        let i = LIGHTS_PATTERNS.iter().position(|p| *p == self.lights_pattern).unwrap_or(0);
        self.lights_pattern = LIGHTS_PATTERNS[(i + 1) % LIGHTS_PATTERNS.len()].to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snow_core::{LightsMode, LightsPattern};

    #[test]
    fn defaults() {
        let s = AppSettings::default();
        assert_eq!(s.intensity, 0.5);
        assert_eq!(s.lights_mode, "off");
        assert_eq!(s.lights_pattern, "twinkle");
        assert!(!s.show_in_capture);
        assert!(!s.paused);
    }

    #[test]
    fn round_trip() {
        let s = AppSettings { intensity: 0.75, lights_mode: "windows".into(), lights_pattern: "chase".into(), show_in_capture: true, paused: true, hotkeys: false };
        assert_eq!(AppSettings::parse(&s.to_json()), s);
    }

    #[test]
    fn hotkeys_default_on_and_round_trip() {
        assert!(AppSettings::default().hotkeys);
        assert!(AppSettings::parse(r#"{"intensity":0.2}"#).hotkeys, "older files have no such field");
        let off = AppSettings { hotkeys: false, ..AppSettings::default() };
        assert!(!AppSettings::parse(&off.to_json()).hotkeys);
    }

    #[test]
    fn garbage_gives_defaults() {
        assert_eq!(AppSettings::parse("{not json"), AppSettings::default());
        assert_eq!(AppSettings::parse(""), AppSettings::default());
        assert_eq!(AppSettings::parse("[1,2,3]"), AppSettings::default());
        assert_eq!(AppSettings::parse("null"), AppSettings::default());
    }

    #[test]
    fn partial_file_keeps_known() {
        let s = AppSettings::parse(r#"{"intensity":0.75}"#);
        assert_eq!(s.intensity, 0.75);
        assert_eq!(s.lights_pattern, "twinkle");
    }

    #[test]
    fn unknown_fields_ignored() {
        assert_eq!(AppSettings::parse(r#"{"intensity":0.25,"future":1}"#).intensity, 0.25);
    }

    #[test]
    fn wrong_type_only_resets_that_field() {
        let s = AppSettings::parse(r#"{"intensity":"lots","lights_mode":"windows","paused":7}"#);
        assert_eq!(s.intensity, 0.5);
        assert_eq!(s.lights_mode, "windows");
        assert!(!s.paused);
    }

    #[test]
    fn out_of_range_is_clamped() {
        assert_eq!(AppSettings::parse(r#"{"intensity":7.0}"#).intensity, 1.0);
        assert_eq!(AppSettings::parse(r#"{"intensity":-1}"#).intensity, 0.0);
    }

    #[test]
    fn bad_enum_strings_default() {
        let s = AppSettings::parse(r#"{"lights_mode":"disco","lights_pattern":"strobe"}"#);
        assert_eq!(s.lights_mode, "off");
        assert_eq!(s.lights_pattern, "twinkle");
        assert_eq!(s.to_core().lights_mode, LightsMode::Off);
        assert_eq!(s.to_core().lights_pattern, LightsPattern::Twinkle);
    }

    #[test]
    fn to_core_maps_every_mode() {
        let mut s = AppSettings::default();
        s.lights_mode = "screen_top".into();
        s.lights_pattern = "chase".into();
        s.intensity = 0.25;
        let c = s.to_core();
        assert_eq!(c.lights_mode, LightsMode::ScreenTop);
        assert_eq!(c.lights_pattern, LightsPattern::Chase);
        assert_eq!(c.intensity, 0.25);
    }

    #[test]
    fn step_intensity_clamps() {
        let mut s = AppSettings { intensity: 0.95, ..AppSettings::default() };
        s.step_intensity(0.1);
        assert_eq!(s.intensity, 1.0);
        s.intensity = 0.05;
        s.step_intensity(-0.1);
        assert_eq!(s.intensity, 0.0);
    }

    #[test]
    fn step_intensity_lands_on_tenths() {
        let mut s = AppSettings::default();
        for _ in 0..3 {
            s.step_intensity(0.1);
        }
        assert_eq!(s.intensity, 0.8);
    }

    #[test]
    fn next_pattern_cycles() {
        let mut s = AppSettings::default();
        s.next_pattern();
        assert_eq!(s.lights_pattern, "chase");
        s.next_pattern();
        assert_eq!(s.lights_pattern, "steady");
        s.next_pattern();
        assert_eq!(s.lights_pattern, "twinkle");
    }
}
