#![doc = "FocusFlow's deterministic, non-destructive timeline editing core."]

use serde::{Deserialize, Serialize};

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
}
