#![doc = "Deterministic mouse-to-camera timeline engine."]

use focusflow_domain::{
    CameraEasing, CameraPose, CameraSegment, CameraTimeline, FocusReason, MouseEvent,
    MouseEventKind, NormalizedPoint,
};
use serde::Deserialize;
use serde::Serialize;
use std::io::BufRead;

const FULL_FRAME: CameraPose = CameraPose {
    center: NormalizedPoint { x: 0.5, y: 0.5 },
    zoom: 1.0,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectorProfile {
    Clean,
    Dynamic,
    Soft,
}

impl DirectorProfile {
    #[must_use]
    pub fn config(self) -> FocusConfig {
        match self {
            Self::Clean => FocusConfig::default(),
            Self::Dynamic => FocusConfig {
                dwell_threshold_ms: 600,
                enter_duration_ms: 420,
                exit_duration_ms: 560,
                click_hold_ms: 950,
                dwell_zoom: 1.30,
                click_zoom: 1.45,
                merge_window_ms: 420,
                merge_distance: 0.05,
                direct_transition_gap_ms: 850,
                click_cooldown_ms: 180,
                min_pan_duration_ms: 300,
                max_pan_duration_ms: 680,
                pan_duration_per_unit_ms: 1_700.0,
            },
            Self::Soft => FocusConfig {
                dwell_threshold_ms: 900,
                enter_duration_ms: 780,
                exit_duration_ms: 950,
                click_hold_ms: 1_300,
                dwell_zoom: 1.16,
                click_zoom: 1.25,
                merge_window_ms: 700,
                merge_distance: 0.08,
                direct_transition_gap_ms: 1_300,
                click_cooldown_ms: 260,
                min_pan_duration_ms: 550,
                max_pan_duration_ms: 1_050,
                pan_duration_per_unit_ms: 3_000.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FocusConfig {
    pub dwell_threshold_ms: u64,
    pub enter_duration_ms: u64,
    pub exit_duration_ms: u64,
    pub click_hold_ms: u64,
    pub dwell_zoom: f64,
    pub click_zoom: f64,
    pub merge_window_ms: u64,
    pub merge_distance: f64,
    pub direct_transition_gap_ms: u64,
    pub click_cooldown_ms: u64,
    pub min_pan_duration_ms: u64,
    pub max_pan_duration_ms: u64,
    pub pan_duration_per_unit_ms: f64,
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            dwell_threshold_ms: 700,
            enter_duration_ms: 550,
            exit_duration_ms: 700,
            click_hold_ms: 1_100,
            dwell_zoom: 1.24,
            click_zoom: 1.34,
            merge_window_ms: 500,
            merge_distance: 0.06,
            direct_transition_gap_ms: 1_000,
            click_cooldown_ms: 220,
            min_pan_duration_ms: 400,
            max_pan_duration_ms: 820,
            pan_duration_per_unit_ms: 2_000.0,
        }
    }
}

pub trait FocusEngine {
    type Error;

    fn analyze(
        &self,
        duration_ms: u64,
        events: &[MouseEvent],
    ) -> Result<CameraTimeline, Self::Error>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SmartFocusEngine {
    pub config: FocusConfig,
}

#[derive(Debug, Clone, Copy)]
struct Cue {
    time_ms: u64,
    hold_until_ms: u64,
    pose: CameraPose,
    reason: FocusReason,
}

#[derive(Debug, Clone, Copy)]
struct Keyframe {
    time_ms: u64,
    pose: CameraPose,
    reason: FocusReason,
}

impl FocusEngine for SmartFocusEngine {
    type Error = String;

    fn analyze(
        &self,
        duration_ms: u64,
        events: &[MouseEvent],
    ) -> Result<CameraTimeline, Self::Error> {
        if duration_ms == 0 {
            return Ok(CameraTimeline {
                schema_version: 1,
                duration_ms: 0,
                segments: Vec::new(),
            });
        }

        let mut cues = self.click_cues(events);
        cues.extend(self.dwell_cues(events));
        cues.retain(|cue| cue.time_ms < duration_ms);
        cues.sort_by_key(|cue| cue.time_ms);
        let cues = self.merge_cues(cues);
        Ok(self.timeline_from_cues(duration_ms, &cues))
    }
}

impl SmartFocusEngine {
    #[must_use]
    pub fn new(config: FocusConfig) -> Self {
        Self { config }
    }

    #[must_use]
    pub fn for_profile(profile: DirectorProfile) -> Self {
        Self::new(profile.config())
    }

    pub fn analyze_trace<R: BufRead>(&self, reader: R) -> Result<CameraTimeline, String> {
        let mut events = Vec::new();
        let mut duration_ms = 0;
        for (index, line) in reader.lines().enumerate() {
            let line = line.map_err(|error| format!("读取轨迹第 {} 行失败：{error}", index + 1))?;
            if line.trim().is_empty() {
                continue;
            }
            let trace: TraceLine = serde_json::from_str(&line)
                .map_err(|error| format!("轨迹第 {} 行不是有效 JSON：{error}", index + 1))?;
            if trace.line_type == "end" {
                duration_ms = duration_ms.max(trace.time_ms.unwrap_or(0));
                continue;
            }
            if trace.line_type != "mouse" {
                continue;
            }
            let event = trace.into_mouse_event(index + 1)?;
            duration_ms = duration_ms.max(event.time_ms);
            events.push(event);
        }
        self.analyze(duration_ms, &events)
    }

    fn click_cues(&self, events: &[MouseEvent]) -> Vec<Cue> {
        events
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    MouseEventKind::LeftClick | MouseEventKind::RightClick
                )
            })
            .map(|event| Cue {
                time_ms: event.time_ms,
                hold_until_ms: event.time_ms.saturating_add(self.config.click_hold_ms),
                pose: CameraPose {
                    center: constrained_center(event.position, self.config.click_zoom),
                    zoom: self.config.click_zoom,
                },
                reason: FocusReason::Click,
            })
            .collect()
    }

    fn dwell_cues(&self, events: &[MouseEvent]) -> Vec<Cue> {
        let moves: Vec<_> = events
            .iter()
            .filter(|event| event.kind == MouseEventKind::Move)
            .collect();
        moves
            .windows(2)
            .filter_map(|pair| {
                let start = pair[0];
                let end = pair[1];
                let gap = end.time_ms.saturating_sub(start.time_ms);
                if gap < self.config.dwell_threshold_ms {
                    return None;
                }
                let focus_time = start.time_ms.saturating_add(self.config.dwell_threshold_ms);
                Some(Cue {
                    time_ms: focus_time,
                    hold_until_ms: end.time_ms.saturating_sub(100).max(focus_time + 350),
                    pose: CameraPose {
                        center: constrained_center(start.position, self.config.dwell_zoom),
                        zoom: self.config.dwell_zoom,
                    },
                    reason: FocusReason::Dwell,
                })
            })
            .collect()
    }

    fn merge_cues(&self, cues: Vec<Cue>) -> Vec<Cue> {
        let mut merged: Vec<Cue> = Vec::new();
        for cue in cues {
            if let Some(previous) = merged.last_mut() {
                let time_gap = cue.time_ms.saturating_sub(previous.time_ms);
                let close_in_time = time_gap <= self.config.merge_window_ms;
                let close_in_space =
                    distance(previous.pose.center, cue.pose.center) <= self.config.merge_distance;
                let continues_active_focus = cue.time_ms
                    <= previous
                        .hold_until_ms
                        .saturating_add(self.config.direct_transition_gap_ms);
                if close_in_space && (close_in_time || continues_active_focus) {
                    previous.hold_until_ms = previous.hold_until_ms.max(cue.hold_until_ms);
                    if cue.reason == FocusReason::Click {
                        previous.pose = cue.pose;
                        previous.reason = cue.reason;
                    }
                    continue;
                }
                if previous.reason == FocusReason::Click
                    && cue.reason == FocusReason::Click
                    && time_gap < self.config.click_cooldown_ms
                {
                    *previous = cue;
                    continue;
                }
            }
            merged.push(cue);
        }
        merged
    }

    fn timeline_from_cues(&self, duration_ms: u64, cues: &[Cue]) -> CameraTimeline {
        if cues.is_empty() {
            return CameraTimeline {
                schema_version: 1,
                duration_ms,
                segments: Vec::new(),
            };
        }

        let mut frames = vec![Keyframe {
            time_ms: 0,
            pose: FULL_FRAME,
            reason: FocusReason::FullFrame,
        }];
        let mut previous_hold: u64 = 0;
        let mut previous_focus_end: u64 = 0;
        let mut previous_pose = FULL_FRAME;

        for cue in cues {
            let nominal_enter_start = cue
                .time_ms
                .saturating_sub(self.config.enter_duration_ms / 2);
            let direct_transition = previous_pose != FULL_FRAME
                && nominal_enter_start
                    <= previous_hold.saturating_add(self.config.direct_transition_gap_ms);
            let transition_duration = if direct_transition {
                self.pan_duration(previous_pose.center, cue.pose.center)
            } else {
                self.config.enter_duration_ms
            };
            let enter_start = cue.time_ms.saturating_sub(transition_duration / 2);
            if direct_transition {
                let transition_start = enter_start.max(previous_focus_end).min(duration_ms);
                if let Some(last) = frames.last_mut() {
                    last.time_ms = transition_start;
                    last.pose = previous_pose;
                    last.reason = FocusReason::Transition;
                }
            } else {
                if previous_pose != FULL_FRAME {
                    let exit_end = previous_hold
                        .saturating_add(self.config.exit_duration_ms)
                        .min(duration_ms);
                    push_keyframe(
                        &mut frames,
                        Keyframe {
                            time_ms: exit_end,
                            pose: FULL_FRAME,
                            reason: FocusReason::FullFrame,
                        },
                    );
                }
                push_keyframe(
                    &mut frames,
                    Keyframe {
                        time_ms: enter_start.min(duration_ms),
                        pose: FULL_FRAME,
                        reason: FocusReason::FullFrame,
                    },
                );
            }

            let focus_end = cue
                .time_ms
                .saturating_add(transition_duration / 2)
                .max(
                    frames
                        .last()
                        .map_or(0, |frame| frame.time_ms)
                        .saturating_add(1),
                )
                .min(duration_ms);
            push_keyframe(
                &mut frames,
                Keyframe {
                    time_ms: focus_end,
                    pose: cue.pose,
                    reason: cue.reason,
                },
            );
            previous_focus_end = focus_end;
            let last_safe_hold = duration_ms.saturating_sub(self.config.exit_duration_ms);
            previous_hold = cue
                .hold_until_ms
                .min(last_safe_hold)
                .max(focus_end)
                .min(duration_ms);
            push_keyframe(
                &mut frames,
                Keyframe {
                    time_ms: previous_hold,
                    pose: cue.pose,
                    reason: cue.reason,
                },
            );
            previous_pose = cue.pose;
        }

        if previous_pose != FULL_FRAME && previous_hold < duration_ms {
            push_keyframe(
                &mut frames,
                Keyframe {
                    time_ms: previous_hold
                        .saturating_add(self.config.exit_duration_ms)
                        .min(duration_ms),
                    pose: FULL_FRAME,
                    reason: FocusReason::FullFrame,
                },
            );
        }
        let final_pose = frames.last().map_or(FULL_FRAME, |frame| frame.pose);
        push_keyframe(
            &mut frames,
            Keyframe {
                time_ms: duration_ms,
                pose: final_pose,
                reason: FocusReason::FullFrame,
            },
        );

        let segments = frames
            .windows(2)
            .filter(|pair| pair[1].time_ms > pair[0].time_ms)
            .map(|pair| CameraSegment {
                start_ms: pair[0].time_ms,
                end_ms: pair[1].time_ms,
                from: pair[0].pose,
                to: pair[1].pose,
                easing: if pair[0].pose == pair[1].pose {
                    CameraEasing::Linear
                } else {
                    CameraEasing::EaseInOutCubic
                },
                reason: pair[1].reason,
            })
            .collect();

        CameraTimeline {
            schema_version: 1,
            duration_ms,
            segments,
        }
    }

    fn pan_duration(&self, from: NormalizedPoint, to: NormalizedPoint) -> u64 {
        let distance_duration =
            (distance(from, to) * self.config.pan_duration_per_unit_ms).round() as u64;
        distance_duration.clamp(
            self.config.min_pan_duration_ms,
            self.config.max_pan_duration_ms,
        )
    }
}

#[derive(Deserialize)]
struct TraceLine {
    #[serde(rename = "type")]
    line_type: String,
    time_ms: Option<u64>,
    x: Option<i32>,
    y: Option<i32>,
    normalized_x: Option<f64>,
    normalized_y: Option<f64>,
    speed_px_s: Option<f64>,
    event: Option<String>,
}

impl TraceLine {
    fn into_mouse_event(self, line_number: usize) -> Result<MouseEvent, String> {
        let event = required(self.event, "event", line_number)?;
        let kind = match event.as_str() {
            "move" => MouseEventKind::Move,
            "left_click" => MouseEventKind::LeftClick,
            "right_click" => MouseEventKind::RightClick,
            _ => return Err(format!("轨迹第 {line_number} 行包含未知事件：{event}")),
        };
        Ok(MouseEvent {
            time_ms: required(self.time_ms, "time_ms", line_number)?,
            x_px: required(self.x, "x", line_number)?,
            y_px: required(self.y, "y", line_number)?,
            position: NormalizedPoint {
                x: required(self.normalized_x, "normalized_x", line_number)?,
                y: required(self.normalized_y, "normalized_y", line_number)?,
            },
            speed_px_s: required(self.speed_px_s, "speed_px_s", line_number)?,
            kind,
        })
    }
}

fn required<T>(value: Option<T>, field: &str, line_number: usize) -> Result<T, String> {
    value.ok_or_else(|| format!("轨迹第 {line_number} 行缺少 {field}"))
}

fn constrained_center(point: NormalizedPoint, zoom: f64) -> NormalizedPoint {
    let half = 0.5 / zoom;
    NormalizedPoint {
        x: point.x.clamp(half, 1.0 - half),
        y: point.y.clamp(half, 1.0 - half),
    }
}

fn distance(a: NormalizedPoint, b: NormalizedPoint) -> f64 {
    (a.x - b.x).hypot(a.y - b.y)
}

fn push_keyframe(frames: &mut Vec<Keyframe>, frame: Keyframe) {
    if let Some(last) = frames.last_mut() {
        if frame.time_ms < last.time_ms {
            return;
        }
        if frame.time_ms == last.time_ms {
            *last = frame;
            return;
        }
    }
    frames.push(frame);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mouse(time_ms: u64, x: f64, y: f64, speed: f64, kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            time_ms,
            x_px: 0,
            y_px: 0,
            position: NormalizedPoint { x, y },
            speed_px_s: speed,
            kind,
        }
    }

    #[test]
    fn rapid_motion_does_not_create_focus() {
        let events = vec![
            mouse(0, 0.1, 0.1, 4_000.0, MouseEventKind::Move),
            mouse(16, 0.5, 0.5, 5_000.0, MouseEventKind::Move),
            mouse(32, 0.9, 0.9, 5_000.0, MouseEventKind::Move),
        ];
        let timeline = SmartFocusEngine::default().analyze(2_000, &events).unwrap();
        assert!(timeline.segments.is_empty());
    }

    #[test]
    fn dwell_creates_gentle_focus_and_restore() {
        let events = vec![
            mouse(100, 0.3, 0.4, 100.0, MouseEventKind::Move),
            mouse(2_100, 0.31, 0.4, 80.0, MouseEventKind::Move),
        ];
        let timeline = SmartFocusEngine::default().analyze(4_000, &events).unwrap();
        assert!(
            timeline
                .segments
                .iter()
                .any(|segment| segment.reason == FocusReason::Dwell)
        );
        assert!(
            timeline
                .segments
                .iter()
                .any(|segment| segment.to.zoom == 1.0)
        );
    }

    #[test]
    fn click_has_stronger_zoom() {
        let events = vec![mouse(1_000, 0.6, 0.5, 0.0, MouseEventKind::LeftClick)];
        let timeline = SmartFocusEngine::default().analyze(3_000, &events).unwrap();
        assert!(
            timeline
                .segments
                .iter()
                .any(|segment| segment.to.zoom == 1.34)
        );
    }

    #[test]
    fn nearby_click_and_dwell_merge() {
        let events = vec![
            mouse(100, 0.6, 0.5, 50.0, MouseEventKind::Move),
            mouse(850, 0.6, 0.5, 0.0, MouseEventKind::LeftClick),
            mouse(2_000, 0.61, 0.5, 40.0, MouseEventKind::Move),
        ];
        let timeline = SmartFocusEngine::default().analyze(3_500, &events).unwrap();
        let click_segments = timeline
            .segments
            .iter()
            .filter(|segment| segment.reason == FocusReason::Click)
            .count();
        assert!(click_segments >= 1);
        assert!(
            timeline
                .segments
                .iter()
                .all(|segment| segment.to.zoom <= 1.34)
        );
    }

    #[test]
    fn consecutive_distant_clicks_pan_without_instant_jump() {
        let events = vec![
            mouse(1_000, 0.25, 0.5, 0.0, MouseEventKind::LeftClick),
            mouse(1_600, 0.75, 0.5, 0.0, MouseEventKind::LeftClick),
        ];
        let timeline = SmartFocusEngine::default().analyze(4_000, &events).unwrap();
        let pans: Vec<_> = timeline
            .segments
            .iter()
            .filter(|segment| {
                segment.from.zoom > 1.0
                    && segment.to.zoom > 1.0
                    && segment.from.center != segment.to.center
            })
            .collect();
        assert_eq!(pans.len(), 1);
        assert!(pans[0].end_ms - pans[0].start_ms >= 300);
    }

    #[test]
    fn repeated_clicks_in_same_area_extend_one_focus_and_restore() {
        let events = vec![
            mouse(1_000, 0.55, 0.4, 0.0, MouseEventKind::LeftClick),
            mouse(2_500, 0.55, 0.4, 0.0, MouseEventKind::LeftClick),
            mouse(4_000, 0.55, 0.4, 0.0, MouseEventKind::LeftClick),
        ];
        let timeline = SmartFocusEngine::default().analyze(6_000, &events).unwrap();
        let focus_entries = timeline
            .segments
            .iter()
            .filter(|segment| segment.from.zoom == 1.0 && segment.to.zoom > 1.0)
            .count();
        assert_eq!(focus_entries, 1);
        assert_eq!(timeline.segments.last().unwrap().to, FULL_FRAME);
    }

    #[test]
    fn director_profiles_have_distinct_motion_character() {
        let clean = DirectorProfile::Clean.config();
        let dynamic = DirectorProfile::Dynamic.config();
        let soft = DirectorProfile::Soft.config();

        assert!(dynamic.click_zoom > clean.click_zoom);
        assert!(clean.click_zoom > soft.click_zoom);
        assert!(dynamic.enter_duration_ms < clean.enter_duration_ms);
        assert!(clean.enter_duration_ms < soft.enter_duration_ms);
        assert!(dynamic.exit_duration_ms < soft.exit_duration_ms);
    }

    #[test]
    fn distant_targets_receive_longer_pan_than_nearby_targets() {
        fn pan_duration_for(second_x: f64) -> u64 {
            let events = vec![
                mouse(1_000, 0.25, 0.5, 0.0, MouseEventKind::LeftClick),
                mouse(2_400, second_x, 0.5, 0.0, MouseEventKind::LeftClick),
            ];
            SmartFocusEngine::for_profile(DirectorProfile::Clean)
                .analyze(5_000, &events)
                .unwrap()
                .segments
                .into_iter()
                .find(|segment| {
                    segment.from.zoom > 1.0
                        && segment.to.zoom > 1.0
                        && segment.from.center != segment.to.center
                })
                .map(|segment| segment.end_ms - segment.start_ms)
                .unwrap()
        }

        assert!(pan_duration_for(0.75) > pan_duration_for(0.50));
    }

    #[test]
    fn rapid_distant_clicks_collapse_to_latest_intent() {
        let events = vec![
            mouse(1_000, 0.2, 0.5, 0.0, MouseEventKind::LeftClick),
            mouse(1_100, 0.8, 0.5, 0.0, MouseEventKind::LeftClick),
        ];
        let timeline = SmartFocusEngine::for_profile(DirectorProfile::Clean)
            .analyze(3_500, &events)
            .unwrap();
        let focused_centers: Vec<_> = timeline
            .segments
            .iter()
            .filter(|segment| segment.to.zoom > 1.0)
            .map(|segment| segment.to.center)
            .collect();

        assert!(!focused_centers.is_empty());
        assert!(focused_centers.iter().all(|center| center.x > 0.5));
    }
}
