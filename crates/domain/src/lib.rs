#![doc = "Shared, platform-independent FocusFlow domain types."]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NormalizedPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CameraPose {
    pub center: NormalizedPoint,
    pub zoom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MouseEventKind {
    Move,
    LeftDown,
    LeftUp,
    LeftClick,
    RightDown,
    RightUp,
    RightClick,
    KeyActivity,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MouseEvent {
    pub time_ms: u64,
    pub x_px: i32,
    pub y_px: i32,
    pub position: NormalizedPoint,
    pub speed_px_s: f64,
    pub kind: MouseEventKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CameraEasing {
    Linear,
    EaseInOutCubic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FocusReason {
    FullFrame,
    Dwell,
    Click,
    Drag,
    Typing,
    Transition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub from: CameraPose,
    pub to: CameraPose,
    pub easing: CameraEasing,
    pub reason: FocusReason,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CameraTimeline {
    pub schema_version: u32,
    pub duration_ms: u64,
    pub segments: Vec<CameraSegment>,
}
