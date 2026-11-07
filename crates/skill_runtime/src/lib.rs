#![doc = "Safe, declarative BreadSkill schema and validation boundary."]

use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub const BREADSKILL_SCHEMA_VERSION: u32 = 1;
pub const CLEAN_SKILL_JSON: &str = include_str!("../assets/clean.breadskill.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BreadSkill {
    pub schema_version: u32,
    pub id: String,
    pub version: String,
    pub name: BTreeMap<String, String>,
    #[serde(default)]
    pub description: BTreeMap<String, String>,
    pub author: String,
    #[serde(default)]
    pub capabilities: Vec<SkillCapability>,
    pub director: DirectorRules,
    #[serde(default)]
    pub visual: VisualRules,
    #[serde(default)]
    pub cursor: CursorRules,
    #[serde(default)]
    pub editing: EditingRules,
    #[serde(default)]
    pub export: ExportRules,
    #[serde(default)]
    pub model: ModelRules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillCapability {
    CameraDirector,
    CanvasStyle,
    CursorStyle,
    IdleEditing,
    Captions,
    Audio,
    ModelReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DirectorRules {
    pub dwell_threshold_ms: u64,
    pub enter_duration_ms: u64,
    pub exit_duration_ms: u64,
    pub click_hold_ms: u64,
    pub double_click_hold_ms: u64,
    pub drag_hold_ms: u64,
    pub typing_hold_ms: u64,
    pub scroll_hold_ms: u64,
    pub dwell_zoom: f64,
    pub click_zoom: f64,
    pub double_click_zoom: f64,
    pub drag_zoom: f64,
    pub typing_zoom: f64,
    pub scroll_zoom: f64,
    pub merge_window_ms: u64,
    pub merge_distance: f64,
    pub direct_transition_gap_ms: u64,
    pub click_cooldown_ms: u64,
    pub min_pan_duration_ms: u64,
    pub max_pan_duration_ms: u64,
    pub pan_duration_per_unit_ms: f64,
    pub min_confidence: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualRules {
    pub background: String,
    pub padding: u32,
    pub corner_radius: u32,
    pub shadow: String,
}

impl Default for VisualRules {
    fn default() -> Self {
        Self { background: "none".to_string(), padding: 0, corner_radius: 0, shadow: "none".to_string() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CursorRules {
    pub scale_percent: u32,
    pub click_effect: String,
}

impl Default for CursorRules {
    fn default() -> Self {
        Self { scale_percent: 100, click_effect: "none".to_string() }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct EditingRules {
    pub trim_idle: bool,
    pub idle_threshold_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportRules {
    pub aspect_ratio: String,
    pub resolution: String,
    pub fps: u32,
}

impl Default for ExportRules {
    fn default() -> Self {
        Self { aspect_ratio: "source".to_string(), resolution: "source".to_string(), fps: 60 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelRules {
    pub required: bool,
    pub capability: Option<String>,
    pub fallback: String,
}

impl Default for ModelRules {
    fn default() -> Self {
        Self { required: false, capability: None, fallback: "local_rules".to_string() }
    }
}

impl BreadSkill {
    pub fn from_json(json: &str) -> Result<Self, String> {
        let skill: Self = serde_json::from_str(json)
            .map_err(|error| format!("invalid BreadSkill JSON: {error}"))?;
        skill.validate()?;
        Ok(skill)
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let json = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read BreadSkill: {error}"))?;
        Self::from_json(&json)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != BREADSKILL_SCHEMA_VERSION {
            return Err(format!("unsupported BreadSkill schema: {}", self.schema_version));
        }
        if !valid_skill_id(&self.id) {
            return Err("BreadSkill id must use lowercase letters, digits, dots, hyphens or underscores".to_string());
        }
        if self.version.trim().is_empty() || self.author.trim().is_empty() || self.name.is_empty() {
            return Err("BreadSkill version, author and name are required".to_string());
        }
        let director = self.director;
        for (name, zoom) in [
            ("dwell_zoom", director.dwell_zoom), ("click_zoom", director.click_zoom),
            ("double_click_zoom", director.double_click_zoom), ("drag_zoom", director.drag_zoom),
            ("typing_zoom", director.typing_zoom), ("scroll_zoom", director.scroll_zoom),
        ] {
            if !zoom.is_finite() || !(1.0..=6.0).contains(&zoom) {
                return Err(format!("{name} must be between 1.0 and 6.0"));
            }
        }
        if !director.min_confidence.is_finite() || !(0.0..=1.0).contains(&director.min_confidence) {
            return Err("min_confidence must be between 0 and 1".to_string());
        }
        if director.min_pan_duration_ms > director.max_pan_duration_ms {
            return Err("min_pan_duration_ms cannot exceed max_pan_duration_ms".to_string());
        }
        if self.cursor.scale_percent < 25 || self.cursor.scale_percent > 400 {
            return Err("cursor scale_percent must be between 25 and 400".to_string());
        }
        if self.export.fps == 0 || self.export.fps > 240 {
            return Err("export fps must be between 1 and 240".to_string());
        }
        if self.model.required && self.model.capability.as_deref().is_none_or(str::is_empty) {
            return Err("a required model must declare a capability".to_string());
        }
        Ok(())
    }
}

fn valid_skill_id(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_'))
}

pub fn clean_skill() -> BreadSkill {
    BreadSkill::from_json(CLEAN_SKILL_JSON).expect("built-in Clean BreadSkill must be valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_clean_skill_is_valid() {
        let skill = clean_skill();
        assert_eq!(skill.id, "breadshot.clean");
        assert!(skill.capabilities.contains(&SkillCapability::CameraDirector));
    }

    #[test]
    fn rejects_unsafe_id_and_invalid_zoom() {
        let mut skill = clean_skill();
        skill.id = "../escape".to_string();
        assert!(skill.validate().is_err());
        skill.id = "breadshot.test".to_string();
        skill.director.click_zoom = 99.0;
        assert!(skill.validate().is_err());
    }

    #[test]
    fn rejects_required_model_without_capability() {
        let mut skill = clean_skill();
        skill.model.required = true;
        skill.model.capability = None;
        assert!(skill.validate().is_err());
    }
}
