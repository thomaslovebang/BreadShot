#![doc = "FocusFlow's deterministic, non-destructive timeline editing core."]

use focusflow_domain::{
    CameraEasing, CameraPose, CameraSegment, CameraTimeline, FocusReason, NormalizedPoint,
};
use serde::{Deserialize, Serialize};

/// One editable point on the virtual-camera path.
///
/// The segment that arrives at this keyframe uses its pose, easing, and reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CameraKeyframe {
    pub time_ms: u64,
    pub pose: CameraPose,
    pub easing: CameraEasing,
    pub reason: FocusReason,
}

/// Converts a camera timeline into editable keyframes.
///
/// Empty automatic timelines become a full-frame hold so the workbench can add zoom.
#[must_use]
pub fn keyframes_from_timeline(timeline: &CameraTimeline) -> Vec<CameraKeyframe> {
    let full = full_frame();
    if timeline.segments.is_empty() {
        if timeline.duration_ms == 0 {
            return vec![full_keyframe(0, full)];
        }
        return vec![
            full_keyframe(0, full),
            full_keyframe(timeline.duration_ms, full),
        ];
    }

    let mut frames = Vec::new();
    let first = &timeline.segments[0];
    if first.start_ms > 0 {
        frames.push(full_keyframe(0, first.from));
    }
    frames.push(CameraKeyframe {
        time_ms: first.start_ms,
        pose: first.from,
        easing: CameraEasing::Linear,
        reason: FocusReason::FullFrame,
    });
    for segment in &timeline.segments {
        if frames
            .last()
            .is_some_and(|frame| segment.end_ms <= frame.time_ms)
        {
            continue;
        }
        frames.push(CameraKeyframe {
            time_ms: segment.end_ms,
            pose: segment.to,
            easing: segment.easing,
            reason: segment.reason,
        });
    }
    if let Some(last) = frames.last().cloned() {
        if last.time_ms < timeline.duration_ms {
            frames.push(full_keyframe(timeline.duration_ms, last.pose));
        }
    }
    frames
}

/// Rebuilds a schema-1 camera timeline from workbench keyframes.
///
/// # Errors
///
/// Returns an error when the path has fewer than two points, does not start at
/// zero, is not strictly increasing, or contains a pose outside the renderer range.
pub fn timeline_from_keyframes(frames: &[CameraKeyframe]) -> Result<CameraTimeline, String> {
    if frames.len() < 2 {
        return Err("镜头至少需要两个关键帧".to_string());
    }
    if frames[0].time_ms != 0 {
        return Err("第一个关键帧必须位于起点".to_string());
    }
    for pair in frames.windows(2) {
        if pair[1].time_ms <= pair[0].time_ms {
            return Err("关键帧时间必须递增".to_string());
        }
    }
    for frame in frames {
        validate_pose(&frame.pose)?;
    }
    let duration_ms = frames.last().map_or(0, |frame| frame.time_ms);
    if duration_ms == 0 {
        return Err("镜头时长无效".to_string());
    }
    let segments = frames
        .windows(2)
        .map(|pair| CameraSegment {
            start_ms: pair[0].time_ms,
            end_ms: pair[1].time_ms,
            from: pair[0].pose,
            to: pair[1].pose,
            easing: pair[1].easing,
            reason: pair[1].reason,
        })
        .collect();
    Ok(CameraTimeline {
        schema_version: 1,
        duration_ms,
        segments,
    })
}

/// Samples the camera pose at `time_ms`, linearly between surrounding keyframes.
///
/// # Errors
///
/// Returns an error when `frames` is empty.
pub fn sample_pose(frames: &[CameraKeyframe], time_ms: u64) -> Result<CameraPose, String> {
    let Some(first) = frames.first() else {
        return Err("没有镜头关键帧".to_string());
    };
    if time_ms <= first.time_ms {
        return Ok(first.pose);
    }
    let Some(last) = frames.last() else {
        return Err("没有镜头关键帧".to_string());
    };
    if time_ms >= last.time_ms {
        return Ok(last.pose);
    }
    let index = frames
        .iter()
        .position(|frame| frame.time_ms > time_ms)
        .ok_or_else(|| "无法采样镜头".to_string())?;
    let left = &frames[index - 1];
    let right = &frames[index];
    let span = right.time_ms.saturating_sub(left.time_ms);
    if span == 0 {
        return Ok(right.pose);
    }
    #[allow(clippy::cast_precision_loss)]
    let progress = (time_ms - left.time_ms) as f64 / span as f64;
    Ok(CameraPose {
        center: NormalizedPoint {
            x: lerp(left.pose.center.x, right.pose.center.x, progress),
            y: lerp(left.pose.center.y, right.pose.center.y, progress),
        },
        zoom: lerp(left.pose.zoom, right.pose.zoom, progress),
    })
}

fn full_frame() -> CameraPose {
    CameraPose {
        center: NormalizedPoint { x: 0.5, y: 0.5 },
        zoom: 1.0,
    }
}

fn full_keyframe(time_ms: u64, pose: CameraPose) -> CameraKeyframe {
    CameraKeyframe {
        time_ms,
        pose,
        easing: CameraEasing::Linear,
        reason: FocusReason::FullFrame,
    }
}

fn validate_pose(pose: &CameraPose) -> Result<(), String> {
    if !(pose.center.x.is_finite() && pose.center.y.is_finite() && pose.zoom.is_finite()) {
        return Err("镜头参数无效".to_string());
    }
    if !(0.0..=1.0).contains(&pose.center.x)
        || !(0.0..=1.0).contains(&pose.center.y)
        || !(1.0..=3.0).contains(&pose.zoom)
    {
        return Err("镜头参数超出安全范围".to_string());
    }
    Ok(())
}

fn lerp(from: f64, to: f64, progress: f64) -> f64 {
    from + (to - from) * progress
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoClip {
    pub id: String,
    pub source_start_ms: u64,
    pub source_end_ms: u64,
}

impl VideoClip {
    #[must_use]
    pub const fn duration_ms(&self) -> u64 {
        self.source_end_ms.saturating_sub(self.source_start_ms)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timeline {
    pub clips: Vec<VideoClip>,
}

impl Timeline {
    #[must_use]
    pub fn from_source(duration_ms: u64) -> Self {
        Self {
            clips: vec![VideoClip {
                id: "clip-1".to_string(),
                source_start_ms: 0,
                source_end_ms: duration_ms,
            }],
        }
    }

    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.clips.iter().map(VideoClip::duration_ms).sum()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.clips.is_empty() {
            return Err("timeline must contain at least one clip".to_string());
        }
        for clip in &self.clips {
            if clip.id.trim().is_empty() || clip.source_start_ms >= clip.source_end_ms {
                return Err("timeline contains an invalid clip".to_string());
            }
        }
        Ok(())
    }

    pub fn split(&mut self, clip_id: &str, offset_ms: u64) -> Result<String, String> {
        let index = self
            .clips
            .iter()
            .position(|clip| clip.id == clip_id)
            .ok_or_else(|| "clip not found".to_string())?;
        let clip = self.clips[index].clone();
        if offset_ms == 0 || offset_ms >= clip.duration_ms() {
            return Err("split point must be inside the clip".to_string());
        }
        let split_source_ms = clip.source_start_ms + offset_ms;
        let right_id = next_clip_id(&self.clips);
        self.clips[index].source_end_ms = split_source_ms;
        self.clips.insert(
            index + 1,
            VideoClip {
                id: right_id.clone(),
                source_start_ms: split_source_ms,
                source_end_ms: clip.source_end_ms,
            },
        );
        Ok(right_id)
    }

    pub fn ripple_delete(&mut self, clip_id: &str) -> Result<VideoClip, String> {
        if self.clips.len() == 1 {
            return Err("cannot delete the only clip".to_string());
        }
        let index = self
            .clips
            .iter()
            .position(|clip| clip.id == clip_id)
            .ok_or_else(|| "clip not found".to_string())?;
        Ok(self.clips.remove(index))
    }

    pub fn move_clip(&mut self, clip_id: &str, target_index: usize) -> Result<(), String> {
        let index = self
            .clips
            .iter()
            .position(|clip| clip.id == clip_id)
            .ok_or_else(|| "clip not found".to_string())?;
        let clip = self.clips.remove(index);
        self.clips.insert(target_index.min(self.clips.len()), clip);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct History {
    present: Timeline,
    undo: Vec<Timeline>,
    redo: Vec<Timeline>,
}

impl History {
    #[must_use]
    pub const fn new(timeline: Timeline) -> Self {
        Self {
            present: timeline,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    #[must_use]
    pub const fn present(&self) -> &Timeline {
        &self.present
    }

    pub fn apply<F>(&mut self, operation: F) -> Result<(), String>
    where
        F: FnOnce(&mut Timeline) -> Result<(), String>,
    {
        let previous = self.present.clone();
        operation(&mut self.present)?;
        self.present.validate()?;
        self.undo.push(previous);
        self.redo.clear();
        Ok(())
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.present, previous));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(&mut self.present, next));
        true
    }
}

fn next_clip_id(clips: &[VideoClip]) -> String {
    let mut suffix = clips.len() + 1;
    loop {
        let candidate = format!("clip-{suffix}");
        if clips.iter().all(|clip| clip.id != candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use focusflow_domain::{
        CameraEasing, CameraPose, CameraSegment, CameraTimeline, FocusReason, NormalizedPoint,
    };

    #[test]
    fn split_preserves_total_duration() {
        let mut timeline = Timeline::from_source(10_000);
        timeline.split("clip-1", 4_000).unwrap();
        assert_eq!(timeline.duration_ms(), 10_000);
        assert_eq!(timeline.clips.len(), 2);
        assert_eq!(timeline.clips[1].source_start_ms, 4_000);
    }

    #[test]
    fn ripple_delete_closes_the_timeline() {
        let mut timeline = Timeline::from_source(10_000);
        let right = timeline.split("clip-1", 4_000).unwrap();
        timeline.ripple_delete(&right).unwrap();
        assert_eq!(timeline.duration_ms(), 4_000);
    }

    #[test]
    fn undo_and_redo_restore_exact_snapshots() {
        let mut history = History::new(Timeline::from_source(10_000));
        history
            .apply(|timeline| timeline.split("clip-1", 5_000).map(|_| ()))
            .unwrap();
        assert_eq!(history.present().clips.len(), 2);
        assert!(history.undo());
        assert_eq!(history.present().clips.len(), 1);
        assert!(history.redo());
        assert_eq!(history.present().clips.len(), 2);
    }

    #[test]
    fn moving_clip_keeps_media_spans_intact() {
        let mut timeline = Timeline::from_source(9_000);
        let second = timeline.split("clip-1", 3_000).unwrap();
        timeline.move_clip(&second, 0).unwrap();
        assert_eq!(timeline.clips[0].source_start_ms, 3_000);
        assert_eq!(timeline.duration_ms(), 9_000);
    }

    fn pose(x: f64, y: f64, zoom: f64) -> CameraPose {
        CameraPose {
            center: NormalizedPoint { x, y },
            zoom,
        }
    }

    #[test]
    fn empty_timeline_becomes_a_full_frame_hold() {
        let frames = keyframes_from_timeline(&CameraTimeline {
            schema_version: 1,
            duration_ms: 4_000,
            segments: Vec::new(),
        });
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].time_ms, 0);
        assert_eq!(frames[1].time_ms, 4_000);
        assert!((frames[1].pose.zoom - 1.0).abs() < 1e-9);
    }

    #[test]
    fn keyframes_round_trip_through_segments() {
        let timeline = CameraTimeline {
            schema_version: 1,
            duration_ms: 2_000,
            segments: vec![
                CameraSegment {
                    start_ms: 0,
                    end_ms: 800,
                    from: pose(0.5, 0.5, 1.0),
                    to: pose(0.4, 0.6, 1.34),
                    easing: CameraEasing::EaseInOutCubic,
                    reason: FocusReason::Click,
                },
                CameraSegment {
                    start_ms: 800,
                    end_ms: 2_000,
                    from: pose(0.4, 0.6, 1.34),
                    to: pose(0.5, 0.5, 1.0),
                    easing: CameraEasing::EaseInOutCubic,
                    reason: FocusReason::FullFrame,
                },
            ],
        };
        let frames = keyframes_from_timeline(&timeline);
        let rebuilt = timeline_from_keyframes(&frames).unwrap();
        assert_eq!(rebuilt.duration_ms, 2_000);
        assert_eq!(rebuilt.segments.len(), 2);
        assert!((rebuilt.segments[0].to.zoom - 1.34).abs() < 1e-9);
        assert_eq!(rebuilt.segments[0].reason, FocusReason::Click);
        assert_eq!(rebuilt.segments[1].end_ms, 2_000);
    }

    #[test]
    fn sample_pose_interpolates_between_keyframes() {
        let frames = vec![
            CameraKeyframe {
                time_ms: 0,
                pose: pose(0.0, 0.0, 1.0),
                easing: CameraEasing::Linear,
                reason: FocusReason::FullFrame,
            },
            CameraKeyframe {
                time_ms: 1_000,
                pose: pose(1.0, 0.5, 2.0),
                easing: CameraEasing::Linear,
                reason: FocusReason::Transition,
            },
        ];
        let sampled = sample_pose(&frames, 500).unwrap();
        assert!((sampled.center.x - 0.5).abs() < 1e-9);
        assert!((sampled.center.y - 0.25).abs() < 1e-9);
        assert!((sampled.zoom - 1.5).abs() < 1e-9);
    }

    #[test]
    fn timeline_rejects_a_keyframe_outside_the_safe_range() {
        let frames = vec![
            CameraKeyframe {
                time_ms: 0,
                pose: pose(0.5, 0.5, 1.0),
                easing: CameraEasing::Linear,
                reason: FocusReason::FullFrame,
            },
            CameraKeyframe {
                time_ms: 1_000,
                pose: pose(0.5, 0.5, 4.0),
                easing: CameraEasing::Linear,
                reason: FocusReason::Click,
            },
        ];
        assert!(timeline_from_keyframes(&frames).is_err());
    }
}
