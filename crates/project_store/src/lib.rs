#![doc = "Open, recoverable FocusFlow project persistence."]

use focusflow_domain::CameraTimeline;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Component, Path, PathBuf},
};

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectState {
    Recording,
    Processing,
    Ready,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectAssets {
    pub source_video: PathBuf,
    pub mouse_trace: PathBuf,
    pub camera_timeline: PathBuf,
    pub edit_timeline: PathBuf,
    pub final_video: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusFlowProject {
    pub schema_version: u32,
    pub project_id: String,
    pub title: String,
    pub created_at_unix_ms: u128,
    pub platform: String,
    pub state: ProjectState,
    pub duration_ms: Option<u64>,
    pub director_profile: String,
    pub assets: ProjectAssets,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EditTimeline {
    pub schema_version: u32,
    pub operations: Vec<EditOperation>,
}

impl Default for EditTimeline {
    fn default() -> Self {
        Self {
            schema_version: PROJECT_SCHEMA_VERSION,
            operations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditOperation {
    Trim {
        start_ms: u64,
        end_ms: u64,
    },
    Cut {
        start_ms: u64,
        end_ms: u64,
    },
    Speed {
        start_ms: u64,
        end_ms: u64,
        rate: f64,
    },
    CameraOverride {
        segment_index: usize,
        center_x: f64,
        center_y: f64,
        zoom: f64,
    },
    SetVideoClips {
        clips: Vec<VideoClipEdit>,
    },
    /// Replaces the rendered camera path. The automatic `camera.json` stays intact.
    SetCameraTimeline {
        timeline: CameraTimeline,
    },
}

impl EditTimeline {
    /// Returns the latest manual camera timeline, when the workbench has exported one.
    #[must_use]
    pub fn replaced_camera_timeline(&self) -> Option<&CameraTimeline> {
        self.operations
            .iter()
            .rev()
            .find_map(|operation| match operation {
                EditOperation::SetCameraTimeline { timeline } => Some(timeline),
                _ => None,
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoClipEdit {
    pub id: String,
    pub source_start_ms: u64,
    pub source_end_ms: u64,
}

impl FocusFlowProject {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err(format!(
                "unsupported FocusFlow project schema: {}",
                self.schema_version
            ));
        }
        if self.project_id.trim().is_empty() || self.title.trim().is_empty() {
            return Err("project id and title cannot be empty".to_string());
        }
        if !matches!(self.director_profile.as_str(), "clean" | "dynamic" | "soft") {
            return Err("unknown director profile".to_string());
        }
        for path in [
            &self.assets.source_video,
            &self.assets.mouse_trace,
            &self.assets.camera_timeline,
            &self.assets.edit_timeline,
            &self.assets.final_video,
        ] {
            validate_asset_path(path)?;
        }
        Ok(())
    }
}

pub fn save_project(path: &Path, project: &FocusFlowProject) -> Result<(), String> {
    project.validate()?;
    save_json_atomically(path, project)
}

pub fn load_project(path: &Path) -> Result<FocusFlowProject, String> {
    let file = File::open(path).map_err(|error| format!("cannot open project: {error}"))?;
    let project: FocusFlowProject = serde_json::from_reader(BufReader::new(file))
        .map_err(|error| format!("invalid FocusFlow project: {error}"))?;
    project.validate()?;
    Ok(project)
}

pub fn save_edit_timeline(path: &Path, timeline: &EditTimeline) -> Result<(), String> {
    if timeline.schema_version != PROJECT_SCHEMA_VERSION {
        return Err("unsupported edit timeline schema".to_string());
    }
    save_json_atomically(path, timeline)
}

pub fn load_edit_timeline(path: &Path) -> Result<EditTimeline, String> {
    let file = File::open(path).map_err(|error| format!("cannot open edit timeline: {error}"))?;
    let timeline: EditTimeline = serde_json::from_reader(BufReader::new(file))
        .map_err(|error| format!("invalid edit timeline: {error}"))?;
    if timeline.schema_version != PROJECT_SCHEMA_VERSION {
        return Err("unsupported edit timeline schema".to_string());
    }
    Ok(timeline)
}

fn validate_asset_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() || path.is_absolute() {
        return Err("project asset paths must be non-empty and relative".to_string());
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err("project asset path escapes the project directory".to_string());
    }
    Ok(())
}

fn save_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .map_err(|error| format!("cannot create project directory: {error}"))?;
    let temporary = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("focusflow")
    ));
    let file =
        File::create(&temporary).map_err(|error| format!("cannot create project: {error}"))?;
    let mut writer = BufWriter::new(file);
    serde_json::to_writer_pretty(&mut writer, value)
        .map_err(|error| format!("cannot encode project: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("cannot flush project: {error}"))?;
    writer
        .get_ref()
        .sync_all()
        .map_err(|error| format!("cannot sync project: {error}"))?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("cannot replace project: {error}"))?;
    }
    fs::rename(&temporary, path).map_err(|error| format!("cannot commit project: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn sample() -> FocusFlowProject {
        FocusFlowProject {
            schema_version: PROJECT_SCHEMA_VERSION,
            project_id: "recording-1".to_string(),
            title: "recording-1".to_string(),
            created_at_unix_ms: 1,
            platform: "windows".to_string(),
            state: ProjectState::Ready,
            duration_ms: Some(1_000),
            director_profile: "clean".to_string(),
            assets: ProjectAssets {
                source_video: "recording-1.mp4".into(),
                mouse_trace: "recording-1.mouse.jsonl".into(),
                camera_timeline: "recording-1.camera.json".into(),
                edit_timeline: "recording-1.edit.json".into(),
                final_video: "recording-1.final.mp4".into(),
            },
        }
    }

    #[test]
    fn round_trips_open_project_format() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("focusflow-project-{nonce}.focusflow"));
        save_project(&path, &sample()).unwrap();
        assert_eq!(load_project(&path).unwrap(), sample());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_assets_outside_project_directory() {
        let mut project = sample();
        project.assets.source_video = PathBuf::from("../secret.mp4");
        assert!(project.validate().is_err());
    }

    #[test]
    fn empty_edit_timeline_is_valid_json() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("focusflow-edit-{nonce}.json"));
        save_edit_timeline(&path, &EditTimeline::default()).unwrap();
        let timeline: EditTimeline = serde_json::from_reader(File::open(&path).unwrap()).unwrap();
        assert!(timeline.operations.is_empty());
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn camera_override_round_trips() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("focusflow-override-{nonce}.json"));
        let timeline = EditTimeline {
            schema_version: PROJECT_SCHEMA_VERSION,
            operations: vec![EditOperation::CameraOverride {
                segment_index: 2,
                center_x: 0.4,
                center_y: 0.6,
                zoom: 1.5,
            }],
        };
        save_edit_timeline(&path, &timeline).unwrap();
        assert_eq!(load_edit_timeline(&path).unwrap(), timeline);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn manual_camera_timeline_round_trips_and_replaces_previous() {
        use focusflow_domain::{
            CameraEasing, CameraPose, CameraSegment, CameraTimeline, FocusReason, NormalizedPoint,
        };

        let pose = CameraPose {
            center: NormalizedPoint { x: 0.4, y: 0.6 },
            zoom: 1.4,
        };
        let timeline = CameraTimeline {
            schema_version: 1,
            duration_ms: 1_000,
            segments: vec![CameraSegment {
                start_ms: 0,
                end_ms: 1_000,
                from: pose,
                to: pose,
                easing: CameraEasing::Linear,
                reason: FocusReason::Transition,
            }],
        };
        let edits = EditTimeline {
            schema_version: PROJECT_SCHEMA_VERSION,
            operations: vec![
                EditOperation::CameraOverride {
                    segment_index: 0,
                    center_x: 0.2,
                    center_y: 0.2,
                    zoom: 1.2,
                },
                EditOperation::SetCameraTimeline {
                    timeline: timeline.clone(),
                },
            ],
        };
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("focusflow-manual-camera-{nonce}.json"));
        save_edit_timeline(&path, &edits).unwrap();
        let loaded = load_edit_timeline(&path).unwrap();
        assert_eq!(loaded.replaced_camera_timeline(), Some(&timeline));
        fs::remove_file(path).unwrap();
    }
}
