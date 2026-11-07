#![doc = "Deterministic mouse-to-camera timeline engine."]

use focusflow_domain::{
    CameraEasing, CameraPose, CameraSegment, CameraTimeline, FocusReason, MouseEvent,
    MouseEventKind, NormalizedPoint,
};
use focusflow_skill_runtime::{DirectorRules, clean_skill};
use serde::Deserialize;
use serde::Serialize;
use std::io::BufRead;

const FULL_FRAME: CameraPose = CameraPose {
    center: NormalizedPoint { x: 0.5, y: 0.5 },
    zoom: 1.0,
};

const FAST_CLICK_LOOKBACK_MS: u64 = 100;
const FAST_CLICK_RECENT_MS: u64 = 40;
const FAST_CLICK_LOOKBACK_DISTANCE: f64 = 0.06;
const FAST_CLICK_RECENT_DISTANCE: f64 = 0.02;
const DRAG_MIN_DISTANCE: f64 = 0.025;
const DOUBLE_CLICK_MAX_GAP_MS: u64 = 500;
const DOUBLE_CLICK_MAX_DISTANCE: f64 = 0.035;
const DWELL_MAX_DISTANCE: f64 = 0.035;

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
            Self::Clean => FocusConfig::from(clean_skill().director),
            Self::Dynamic => FocusConfig {
                dwell_threshold_ms: 600,
                enter_duration_ms: 420,
                exit_duration_ms: 560,
                click_hold_ms: 950,
                double_click_hold_ms: 1_150,
                drag_hold_ms: 500,
                typing_hold_ms: 900,
                scroll_hold_ms: 700,
                dwell_zoom: 1.30,
                click_zoom: 1.45,
                double_click_zoom: 1.52,
                drag_zoom: 1.32,
                typing_zoom: 1.32,
                scroll_zoom: 1.30,
                merge_window_ms: 420,
                merge_distance: 0.05,
                direct_transition_gap_ms: 850,
                click_cooldown_ms: 180,
                min_pan_duration_ms: 300,
                max_pan_duration_ms: 680,
                pan_duration_per_unit_ms: 1_700.0,
                min_confidence: 0.58,
            },
            Self::Soft => FocusConfig {
                dwell_threshold_ms: 900,
                enter_duration_ms: 780,
                exit_duration_ms: 950,
                click_hold_ms: 1_300,
                double_click_hold_ms: 1_450,
                drag_hold_ms: 850,
                typing_hold_ms: 1_250,
                scroll_hold_ms: 1_000,
                dwell_zoom: 1.16,
                click_zoom: 1.25,
                double_click_zoom: 1.30,
                drag_zoom: 1.18,
                typing_zoom: 1.16,
                scroll_zoom: 1.15,
                merge_window_ms: 700,
                merge_distance: 0.08,
                direct_transition_gap_ms: 1_300,
                click_cooldown_ms: 260,
                min_pan_duration_ms: 550,
                max_pan_duration_ms: 1_050,
                pan_duration_per_unit_ms: 3_000.0,
                min_confidence: 0.76,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusConfig {
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

impl From<DirectorRules> for FocusConfig {
    fn from(rules: DirectorRules) -> Self {
        Self {
            dwell_threshold_ms: rules.dwell_threshold_ms,
            enter_duration_ms: rules.enter_duration_ms,
            exit_duration_ms: rules.exit_duration_ms,
            click_hold_ms: rules.click_hold_ms,
            double_click_hold_ms: rules.double_click_hold_ms,
            drag_hold_ms: rules.drag_hold_ms,
            typing_hold_ms: rules.typing_hold_ms,
            scroll_hold_ms: rules.scroll_hold_ms,
            dwell_zoom: rules.dwell_zoom,
            click_zoom: rules.click_zoom,
            double_click_zoom: rules.double_click_zoom,
            drag_zoom: rules.drag_zoom,
            typing_zoom: rules.typing_zoom,
            scroll_zoom: rules.scroll_zoom,
            merge_window_ms: rules.merge_window_ms,
            merge_distance: rules.merge_distance,
            direct_transition_gap_ms: rules.direct_transition_gap_ms,
            click_cooldown_ms: rules.click_cooldown_ms,
            min_pan_duration_ms: rules.min_pan_duration_ms,
            max_pan_duration_ms: rules.max_pan_duration_ms,
            pan_duration_per_unit_ms: rules.pan_duration_per_unit_ms,
            min_confidence: rules.min_confidence,
        }
    }
}

impl Default for FocusConfig {
    fn default() -> Self {
        Self {
            dwell_threshold_ms: 700,
            enter_duration_ms: 550,
            exit_duration_ms: 700,
            click_hold_ms: 1_100,
            double_click_hold_ms: 1_300,
            drag_hold_ms: 650,
            typing_hold_ms: 1_050,
            scroll_hold_ms: 850,
            dwell_zoom: 1.24,
            click_zoom: 1.34,
            double_click_zoom: 1.40,
            drag_zoom: 1.25,
            typing_zoom: 1.24,
            scroll_zoom: 1.22,
            merge_window_ms: 500,
            merge_distance: 0.06,
            direct_transition_gap_ms: 1_000,
            click_cooldown_ms: 220,
            min_pan_duration_ms: 400,
            max_pan_duration_ms: 820,
            pan_duration_per_unit_ms: 2_000.0,
            min_confidence: 0.68,
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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FocusCandidate {
    pub time_ms: u64,
    pub hold_until_ms: u64,
    pub anchor: NormalizedPoint,
    pub pose: CameraPose,
    pub reason: FocusReason,
    pub confidence: f64,
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

        let candidates = self.candidates(duration_ms, events);
        Ok(self.timeline_from_candidates(duration_ms, &candidates))
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
        let (duration_ms, events) = read_trace(reader)?;
        self.analyze(duration_ms, &events)
    }

    pub fn analyze_trace_candidates<R: BufRead>(
        &self,
        reader: R,
    ) -> Result<(u64, Vec<FocusCandidate>), String> {
        let (duration_ms, events) = read_trace(reader)?;
        Ok((duration_ms, self.candidates(duration_ms, &events)))
    }

    #[must_use]
    pub fn candidates(&self, duration_ms: u64, events: &[MouseEvent]) -> Vec<FocusCandidate> {
        let drags = drag_gestures(events);
        let double_clicks = double_click_gestures(events, &drags);
        let mut cues = self.click_cues(events, &drags, &double_clicks);
        cues.extend(self.double_click_cues(&double_clicks));
        cues.extend(self.drag_cues(&drags));
        cues.extend(self.typing_cues(events));
        cues.extend(self.scroll_cues(events));
        cues.extend(self.dwell_cues(events));
        cues.retain(|cue| {
            cue.time_ms < duration_ms && cue.confidence >= self.config.min_confidence
        });
        cues.sort_by_key(|cue| cue.time_ms);
        self.merge_cues(cues)
    }

    #[must_use]
    pub fn timeline_from_candidates(
        &self,
        duration_ms: u64,
        candidates: &[FocusCandidate],
    ) -> CameraTimeline {
        self.timeline_from_cues(duration_ms, candidates)
    }

    fn click_cues(
        &self,
        events: &[MouseEvent],
        drags: &[DragGesture],
        double_clicks: &[DoubleClickGesture],
    ) -> Vec<FocusCandidate> {
        events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                matches!(
                    event.kind,
                    MouseEventKind::LeftClick | MouseEventKind::RightClick
                )
            })
            .filter(|(_, event)| {
                !drags.iter().any(|drag| {
                    event.time_ms >= drag.start.time_ms && event.time_ms <= drag.end.time_ms
                })
            })
            .filter(|(_, event)| !is_double_click_member(event, double_clicks))
            .filter(|(index, _)| !click_during_fast_movement(events, *index))
            .map(|(_, event)| FocusCandidate {
                time_ms: event.time_ms,
                hold_until_ms: event.time_ms.saturating_add(self.config.click_hold_ms),
                anchor: event.position,
                pose: CameraPose {
                    center: constrained_center(event.position, self.config.click_zoom),
                    zoom: self.config.click_zoom,
                },
                reason: FocusReason::Click,
                confidence: click_confidence(event),
            })
            .collect()
    }

    fn double_click_cues(&self, gestures: &[DoubleClickGesture]) -> Vec<FocusCandidate> {
        gestures
            .iter()
            .map(|gesture| FocusCandidate {
                time_ms: gesture.first.time_ms,
                hold_until_ms: gesture
                    .second
                    .time_ms
                    .saturating_add(self.config.double_click_hold_ms),
                anchor: gesture.second.position,
                pose: CameraPose {
                    center: constrained_center(
                        gesture.second.position,
                        self.config.double_click_zoom,
                    ),
                    zoom: self.config.double_click_zoom,
                },
                reason: FocusReason::DoubleClick,
                confidence: 0.99,
            })
            .collect()
    }

    fn drag_cues(&self, drags: &[DragGesture]) -> Vec<FocusCandidate> {
        drags
            .iter()
            .flat_map(|drag| {
                let start = FocusCandidate {
                    time_ms: drag.start.time_ms,
                    hold_until_ms: drag.end.time_ms,
                    anchor: drag.start.position,
                    pose: CameraPose {
                        center: constrained_center(drag.start.position, self.config.drag_zoom),
                        zoom: self.config.drag_zoom,
                    },
                    reason: FocusReason::Drag,
                    confidence: 0.96,
                };
                let end = FocusCandidate {
                    time_ms: drag.end.time_ms,
                    hold_until_ms: drag.end.time_ms.saturating_add(self.config.drag_hold_ms),
                    anchor: drag.end.position,
                    pose: CameraPose {
                        center: constrained_center(drag.end.position, self.config.drag_zoom),
                        zoom: self.config.drag_zoom,
                    },
                    reason: FocusReason::Drag,
                    confidence: 0.96,
                };
                [start, end]
            })
            .collect()
    }

    fn typing_cues(&self, events: &[MouseEvent]) -> Vec<FocusCandidate> {
        let mut latest_click: Option<MouseEvent> = None;
        let mut active_anchor: Option<MouseEvent> = None;
        let mut last_activity_ms: Option<u64> = None;
        let mut cues = Vec::new();

        for event in events {
            if matches!(
                event.kind,
                MouseEventKind::LeftClick | MouseEventKind::RightClick
            ) {
                latest_click = Some(*event);
                active_anchor = None;
                last_activity_ms = None;
                continue;
            }
            if event.kind != MouseEventKind::KeyActivity {
                continue;
            }

            let continues_burst = last_activity_ms
                .is_some_and(|last| event.time_ms.saturating_sub(last) <= 1_000);
            if !continues_burst {
                active_anchor = latest_click
                    .filter(|click| event.time_ms.saturating_sub(click.time_ms) <= 2_500);
            }
            let Some(anchor) = active_anchor else {
                last_activity_ms = Some(event.time_ms);
                continue;
            };
            cues.push(FocusCandidate {
                time_ms: event.time_ms,
                hold_until_ms: event.time_ms.saturating_add(self.config.typing_hold_ms),
                anchor: anchor.position,
                pose: CameraPose {
                    center: constrained_center(anchor.position, self.config.typing_zoom),
                    zoom: self.config.typing_zoom,
                },
                reason: FocusReason::Typing,
                confidence: 0.84,
            });
            last_activity_ms = Some(event.time_ms);
        }
        cues
    }

    fn scroll_cues(&self, events: &[MouseEvent]) -> Vec<FocusCandidate> {
        let mut latest_click: Option<MouseEvent> = None;
        let mut active_anchor: Option<MouseEvent> = None;
        let mut last_scroll_ms: Option<u64> = None;
        let mut cues = Vec::new();

        for event in events {
            if matches!(
                event.kind,
                MouseEventKind::LeftClick | MouseEventKind::RightClick
            ) {
                latest_click = Some(*event);
                active_anchor = None;
                last_scroll_ms = None;
                continue;
            }
            if !matches!(
                event.kind,
                MouseEventKind::ScrollUp
                    | MouseEventKind::ScrollDown
                    | MouseEventKind::ScrollLeft
                    | MouseEventKind::ScrollRight
            ) {
                continue;
            }

            let continues_burst = last_scroll_ms
                .is_some_and(|last| event.time_ms.saturating_sub(last) <= 750);
            if !continues_burst {
                active_anchor = latest_click
                    .filter(|click| event.time_ms.saturating_sub(click.time_ms) <= 3_000);
            }
            let Some(anchor) = active_anchor else {
                last_scroll_ms = Some(event.time_ms);
                continue;
            };
            cues.push(FocusCandidate {
                time_ms: event.time_ms,
                hold_until_ms: event.time_ms.saturating_add(self.config.scroll_hold_ms),
                anchor: anchor.position,
                pose: CameraPose {
                    center: constrained_center(anchor.position, self.config.scroll_zoom),
                    zoom: self.config.scroll_zoom,
                },
                reason: FocusReason::Scroll,
                confidence: 0.86,
            });
            last_scroll_ms = Some(event.time_ms);
        }
        cues
    }

    fn dwell_cues(&self, events: &[MouseEvent]) -> Vec<FocusCandidate> {
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
                let movement = distance(start.position, end.position);
                if gap < self.config.dwell_threshold_ms || movement > DWELL_MAX_DISTANCE {
                    return None;
                }
                let duration_score = (gap.saturating_sub(self.config.dwell_threshold_ms) as f64
                    / 1_500.0)
                    .clamp(0.0, 1.0);
                let stability_score = (1.0 - movement / DWELL_MAX_DISTANCE).clamp(0.0, 1.0);
                let focus_time = start.time_ms.saturating_add(self.config.dwell_threshold_ms);
                Some(FocusCandidate {
                    time_ms: focus_time,
                    hold_until_ms: end.time_ms.saturating_sub(100).max(focus_time + 350),
                    anchor: start.position,
                    pose: CameraPose {
                        center: constrained_center(start.position, self.config.dwell_zoom),
                        zoom: self.config.dwell_zoom,
                    },
                    reason: FocusReason::Dwell,
                    confidence: 0.62 + 0.18 * duration_score + 0.18 * stability_score,
                })
            })
            .collect()
    }

    fn merge_cues(&self, cues: Vec<FocusCandidate>) -> Vec<FocusCandidate> {
        let mut merged: Vec<FocusCandidate> = Vec::new();
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
                    if matches!(
                        cue.reason,
                        FocusReason::Click | FocusReason::DoubleClick | FocusReason::Drag
                    ) {
                        previous.pose = cue.pose;
                        previous.anchor = cue.anchor;
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

    fn timeline_from_cues(&self, duration_ms: u64, cues: &[FocusCandidate]) -> CameraTimeline {
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

fn read_trace<R: BufRead>(reader: R) -> Result<(u64, Vec<MouseEvent>), String> {
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
    Ok((duration_ms, events))
}

fn click_during_fast_movement(events: &[MouseEvent], click_index: usize) -> bool {
    let Some(click) = events.get(click_index) else {
        return false;
    };
    let Some(lookback_distance) = movement_distance_before(
        events,
        click_index,
        click.time_ms.saturating_sub(FAST_CLICK_LOOKBACK_MS),
    ) else {
        return false;
    };
    let Some(recent_distance) = movement_distance_before(
        events,
        click_index,
        click.time_ms.saturating_sub(FAST_CLICK_RECENT_MS),
    ) else {
        return false;
    };
    lookback_distance >= FAST_CLICK_LOOKBACK_DISTANCE
        && recent_distance >= FAST_CLICK_RECENT_DISTANCE
}

fn click_confidence(event: &MouseEvent) -> f64 {
    let settled_score = (1.0 - event.speed_px_s / 2_500.0).clamp(0.0, 1.0);
    0.72 + 0.23 * settled_score
}

#[derive(Debug, Clone, Copy)]
struct DoubleClickGesture {
    first: MouseEvent,
    second: MouseEvent,
}

fn double_click_gestures(
    events: &[MouseEvent],
    drags: &[DragGesture],
) -> Vec<DoubleClickGesture> {
    let mut previous: Option<(usize, MouseEvent)> = None;
    let mut gestures = Vec::new();
    for (index, event) in events.iter().copied().enumerate() {
        if !matches!(
            event.kind,
            MouseEventKind::LeftClick | MouseEventKind::RightClick
        ) {
            continue;
        }
        let belongs_to_drag = drags.iter().any(|drag| {
            event.time_ms >= drag.start.time_ms && event.time_ms <= drag.end.time_ms
        });
        if belongs_to_drag || click_during_fast_movement(events, index) {
            previous = None;
            continue;
        }

        if let Some((_, first)) = previous
            && event.kind == first.kind
            && event.time_ms.saturating_sub(first.time_ms) <= DOUBLE_CLICK_MAX_GAP_MS
            && distance(event.position, first.position) <= DOUBLE_CLICK_MAX_DISTANCE
        {
            gestures.push(DoubleClickGesture {
                first,
                second: event,
            });
            previous = None;
        } else {
            previous = Some((index, event));
        }
    }
    gestures
}

fn is_double_click_member(event: &MouseEvent, gestures: &[DoubleClickGesture]) -> bool {
    gestures.iter().any(|gesture| {
        (event.time_ms == gesture.first.time_ms && event.kind == gesture.first.kind)
            || (event.time_ms == gesture.second.time_ms && event.kind == gesture.second.kind)
    })
}

#[derive(Debug, Clone, Copy)]
struct DragGesture {
    start: MouseEvent,
    end: MouseEvent,
}

fn drag_gestures(events: &[MouseEvent]) -> Vec<DragGesture> {
    let mut active_left: Option<MouseEvent> = None;
    let mut gestures = Vec::new();
    for event in events {
        match event.kind {
            MouseEventKind::LeftDown => active_left = Some(*event),
            MouseEventKind::LeftUp => {
                if let Some(start) = active_left.take()
                    && distance(start.position, event.position) >= DRAG_MIN_DISTANCE
                {
                    gestures.push(DragGesture { start, end: *event });
                }
            }
            _ => {}
        }
    }
    gestures
}

fn movement_distance_before(
    events: &[MouseEvent],
    click_index: usize,
    start_ms: u64,
) -> Option<f64> {
    let click = events.get(click_index)?;
    let mut previous = events[..click_index]
        .iter()
        .rev()
        .find(|event| event.kind == MouseEventKind::Move && event.time_ms <= start_ms)
        .map(|event| event.position);
    let mut distance_sum = 0.0;
    let mut samples = 0_u32;
    for event in &events[..click_index] {
        if event.time_ms < start_ms || event.kind != MouseEventKind::Move {
            continue;
        }
        if let Some(last) = previous {
            distance_sum += distance(last, event.position);
            samples += 1;
        }
        previous = Some(event.position);
    }
    if let Some(last) = previous {
        let final_distance = distance(last, click.position);
        if final_distance > 0.0 {
            distance_sum += final_distance;
            samples += 1;
        }
    }
    (samples > 0).then_some(distance_sum)
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
            "left_down" => MouseEventKind::LeftDown,
            "left_up" => MouseEventKind::LeftUp,
            "right_down" => MouseEventKind::RightDown,
            "right_up" => MouseEventKind::RightUp,
            "key_activity" => MouseEventKind::KeyActivity,
            "scroll_up" => MouseEventKind::ScrollUp,
            "scroll_down" => MouseEventKind::ScrollDown,
            "scroll_left" => MouseEventKind::ScrollLeft,
            "scroll_right" => MouseEventKind::ScrollRight,
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

    #[test]
    fn clean_profile_is_driven_by_the_built_in_breadskill() {
        let skill_config = FocusConfig::from(clean_skill().director);
        assert_eq!(DirectorProfile::Clean.config(), skill_config);
        assert_eq!(skill_config, FocusConfig::default());
    }

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
    fn long_gap_with_large_pointer_change_is_not_mistaken_for_dwell() {
        let events = vec![
            mouse(100, 0.20, 0.40, 0.0, MouseEventKind::Move),
            mouse(2_100, 0.70, 0.40, 0.0, MouseEventKind::Move),
        ];
        let timeline = SmartFocusEngine::default().analyze(4_000, &events).unwrap();

        assert!(timeline.segments.is_empty());
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
    fn double_click_becomes_one_stronger_intent() {
        let events = vec![
            mouse(1_000, 0.60, 0.50, 0.0, MouseEventKind::LeftClick),
            mouse(1_280, 0.605, 0.50, 0.0, MouseEventKind::LeftClick),
        ];
        let timeline = SmartFocusEngine::default().analyze(4_000, &events).unwrap();

        assert!(timeline.segments.iter().any(|segment| {
            segment.reason == FocusReason::DoubleClick && segment.to.zoom == 1.40
        }));
        assert!(!timeline
            .segments
            .iter()
            .any(|segment| segment.reason == FocusReason::Click));
        assert_eq!(
            timeline
                .segments
                .iter()
                .filter(|segment| segment.from.zoom == 1.0 && segment.to.zoom > 1.0)
                .count(),
            1
        );
    }

    #[test]
    fn confidence_threshold_prefers_no_zoom_over_a_weak_candidate() {
        let strict = SmartFocusEngine::new(FocusConfig {
            min_confidence: 0.96,
            ..FocusConfig::default()
        });
        let single_click = vec![mouse(
            1_000,
            0.60,
            0.50,
            0.0,
            MouseEventKind::LeftClick,
        )];
        assert!(strict.analyze(3_000, &single_click).unwrap().segments.is_empty());

        let double_click = vec![
            mouse(1_000, 0.60, 0.50, 0.0, MouseEventKind::LeftClick),
            mouse(1_250, 0.60, 0.50, 0.0, MouseEventKind::LeftClick),
        ];
        assert!(strict
            .analyze(3_000, &double_click)
            .unwrap()
            .segments
            .iter()
            .any(|segment| segment.reason == FocusReason::DoubleClick));
    }

    fn has_click_focus(events: &[MouseEvent]) -> bool {
        SmartFocusEngine::default()
            .analyze(3_000, events)
            .unwrap()
            .segments
            .iter()
            .any(|segment| segment.reason == FocusReason::Click && segment.to.zoom > 1.0)
    }

    #[test]
    fn fast_moving_left_click_is_ignored() {
        let events = vec![
            mouse(880, 0.10, 0.50, 0.0, MouseEventKind::Move),
            mouse(920, 0.18, 0.50, 0.0, MouseEventKind::Move),
            mouse(960, 0.27, 0.50, 0.0, MouseEventKind::Move),
            mouse(990, 0.34, 0.50, 0.0, MouseEventKind::Move),
            mouse(1_000, 0.36, 0.50, 0.0, MouseEventKind::LeftClick),
        ];
        assert!(!has_click_focus(&events));
    }

    #[test]
    fn fast_moving_right_click_is_ignored() {
        let events = vec![
            mouse(880, 0.10, 0.50, 0.0, MouseEventKind::Move),
            mouse(920, 0.18, 0.50, 0.0, MouseEventKind::Move),
            mouse(960, 0.27, 0.50, 0.0, MouseEventKind::Move),
            mouse(990, 0.34, 0.50, 0.0, MouseEventKind::Move),
            mouse(1_000, 0.36, 0.50, 0.0, MouseEventKind::RightClick),
        ];
        assert!(!has_click_focus(&events));
    }

    #[test]
    fn slow_moving_click_is_preserved() {
        let events = vec![
            mouse(880, 0.50, 0.50, 0.0, MouseEventKind::Move),
            mouse(930, 0.505, 0.50, 0.0, MouseEventKind::Move),
            mouse(980, 0.510, 0.50, 0.0, MouseEventKind::Move),
            mouse(1_000, 0.512, 0.50, 0.0, MouseEventKind::LeftClick),
        ];
        assert!(has_click_focus(&events));
    }

    #[test]
    fn click_after_fast_movement_has_stopped_is_preserved() {
        let events = vec![
            mouse(700, 0.10, 0.50, 0.0, MouseEventKind::Move),
            mouse(800, 0.50, 0.50, 0.0, MouseEventKind::Move),
            mouse(1_000, 0.50, 0.50, 0.0, MouseEventKind::LeftClick),
        ];
        assert!(has_click_focus(&events));
    }

    #[test]
    fn click_without_enough_motion_history_is_preserved() {
        let events = vec![mouse(1_000, 0.60, 0.50, 0.0, MouseEventKind::LeftClick)];
        assert!(has_click_focus(&events));
    }

    #[test]
    fn drag_pans_camera_from_press_to_release_without_click_zoom() {
        let events = vec![
            mouse(1_000, 0.25, 0.45, 0.0, MouseEventKind::LeftDown),
            mouse(1_000, 0.25, 0.45, 0.0, MouseEventKind::LeftClick),
            mouse(1_350, 0.50, 0.50, 500.0, MouseEventKind::Move),
            mouse(1_700, 0.75, 0.55, 0.0, MouseEventKind::LeftUp),
        ];
        let timeline = SmartFocusEngine::default().analyze(4_000, &events).unwrap();

        assert!(
            timeline
                .segments
                .iter()
                .any(|segment| segment.reason == FocusReason::Drag)
        );
        assert!(timeline
            .segments
            .iter()
            .any(|segment| segment.from.zoom > 1.0
                && segment.to.zoom > 1.0
                && segment.from.center != segment.to.center));
        assert!(!timeline
            .segments
            .iter()
            .any(|segment| segment.reason == FocusReason::Click));
    }

    #[test]
    fn tiny_press_release_movement_remains_a_click() {
        let events = vec![
            mouse(1_000, 0.50, 0.50, 0.0, MouseEventKind::LeftDown),
            mouse(1_000, 0.50, 0.50, 0.0, MouseEventKind::LeftClick),
            mouse(1_080, 0.51, 0.50, 0.0, MouseEventKind::LeftUp),
        ];
        let timeline = SmartFocusEngine::default().analyze(3_000, &events).unwrap();

        assert!(timeline
            .segments
            .iter()
            .any(|segment| segment.reason == FocusReason::Click));
        assert!(!timeline
            .segments
            .iter()
            .any(|segment| segment.reason == FocusReason::Drag));
    }

    #[test]
    fn typing_after_click_keeps_the_clicked_area_focused() {
        let events = vec![
            mouse(1_000, 0.35, 0.45, 0.0, MouseEventKind::LeftClick),
            mouse(1_700, 0.35, 0.45, 0.0, MouseEventKind::KeyActivity),
            mouse(2_400, 0.35, 0.45, 0.0, MouseEventKind::KeyActivity),
            mouse(3_100, 0.35, 0.45, 0.0, MouseEventKind::KeyActivity),
        ];
        let timeline = SmartFocusEngine::default().analyze(5_000, &events).unwrap();
        let restore = timeline
            .segments
            .iter()
            .find(|segment| segment.from.zoom > 1.0 && segment.to.zoom == 1.0)
            .expect("typing sequence should eventually restore full frame");
        assert!(restore.start_ms >= 4_000);

        let click_only = SmartFocusEngine::default()
            .analyze(
                5_000,
                &[mouse(
                    1_000,
                    0.35,
                    0.45,
                    0.0,
                    MouseEventKind::LeftClick,
                )],
            )
            .unwrap();
        let click_only_restore = click_only
            .segments
            .iter()
            .find(|segment| segment.from.zoom > 1.0 && segment.to.zoom == 1.0)
            .expect("click should eventually restore full frame");
        assert!(restore.start_ms > click_only_restore.start_ms);
    }

    #[test]
    fn anonymous_key_activity_without_a_recent_click_does_not_guess_a_target() {
        let events = vec![
            mouse(1_000, 0.50, 0.50, 0.0, MouseEventKind::KeyActivity),
            mouse(1_500, 0.50, 0.50, 0.0, MouseEventKind::KeyActivity),
        ];
        let timeline = SmartFocusEngine::default().analyze(3_000, &events).unwrap();

        assert!(timeline.segments.is_empty());
    }

    #[test]
    fn scrolling_after_click_extends_focus_without_retargeting() {
        let events = vec![
            mouse(1_000, 0.40, 0.45, 0.0, MouseEventKind::LeftClick),
            mouse(1_600, 0.80, 0.80, 0.0, MouseEventKind::ScrollDown),
            mouse(2_100, 0.80, 0.80, 0.0, MouseEventKind::ScrollDown),
            mouse(2_600, 0.80, 0.80, 0.0, MouseEventKind::ScrollUp),
        ];
        let timeline = SmartFocusEngine::default().analyze(5_000, &events).unwrap();
        let restore = timeline
            .segments
            .iter()
            .find(|segment| segment.from.zoom > 1.0 && segment.to.zoom == 1.0)
            .expect("scrolling sequence should eventually restore full frame");
        assert!(restore.start_ms >= 3_400);
        assert!(timeline.segments.iter().filter(|segment| {
            segment.from.zoom == 1.0 && segment.to.zoom > 1.0
        }).count() == 1);
    }

    #[test]
    fn scrolling_without_a_recent_click_does_not_guess_a_target() {
        let events = vec![
            mouse(1_000, 0.95, 0.50, 0.0, MouseEventKind::ScrollDown),
            mouse(1_300, 0.95, 0.50, 0.0, MouseEventKind::ScrollDown),
            mouse(1_600, 0.95, 0.50, 0.0, MouseEventKind::ScrollDown),
        ];
        let timeline = SmartFocusEngine::default().analyze(3_000, &events).unwrap();

        assert!(timeline.segments.is_empty());
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
